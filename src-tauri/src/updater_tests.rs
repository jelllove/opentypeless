// Local HTTP fixtures exercise the native Tauri signature verifier, never install payloads.
use std::io::{Read, Write};
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;

async fn download_fixture(tampered: bool) -> tauri_plugin_updater::Result<Vec<u8>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let signature = include_str!("../tests/fixtures/updater-payload.txt.sig").trim();
    let metadata = serde_json::json!({
        "version": "99.0.0",
        "platforms": { "windows-x86_64": {
            "url": format!("http://{address}/payload"),
            "signature": signature
        }}
    })
    .to_string();
    let mut payload = include_bytes!("../tests/fixtures/updater-payload.txt").to_vec();
    if tampered {
        payload[0] ^= 1;
    }
    let worker = std::thread::spawn(move || {
        for (content_type, body) in [
            ("application/json", metadata.into_bytes()),
            ("application/octet-stream", payload),
        ] {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "fixture request timed out"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let n = socket.read(&mut chunk).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|s| s == b"\r\n\r\n") {
                    break;
                }
            }
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    let production: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let pubkey = production["plugins"]["updater"]["pubkey"].as_str().unwrap();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": pubkey,
            "endpoints": [format!("http://{address}/latest.json")],
            "dangerousInsecureTransportProtocol": true
        }),
    );
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap();
    let updater = app
        .updater_builder()
        .target("windows-x86_64")
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let update = updater.check().await.unwrap().unwrap();
    let result = update.download(|_, _| {}, || {}).await;
    worker.join().unwrap();
    result
}

#[tokio::test]
async fn fork_public_key_accepts_signed_fixture_and_rejects_modified_download() {
    assert_eq!(
        download_fixture(false).await.unwrap(),
        include_bytes!("../tests/fixtures/updater-payload.txt")
    );
    assert!(download_fixture(true).await.is_err());
}
