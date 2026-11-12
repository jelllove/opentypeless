use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use super::{whisper_compat::WhisperCompatProvider, SttConfig, SttProvider, TranscriptEvent};
use crate::error::AppError;

pub const PROVIDER: &str = "azure-speech";
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
// Three upload/read budgets + 1s/2s backoff + 15s finalization margin.
pub const FINALIZE_TIMEOUT: Duration = Duration::from_secs(378);
const MAX_RETRIES: u32 = 2;
const MAX_AUDIO_BYTES: usize = super::capabilities::CLIENT_FILE_BUFFER_BYTES as usize;
const API_VERSION: &str = "2025-10-15";

pub fn transcription_endpoint(base: &str) -> Result<String, AppError> {
    let mut url = url::Url::parse(base.trim()).map_err(|_| {
        AppError::Config("Enter a valid Azure Speech HTTPS resource endpoint".into())
    })?;
    let local_test = cfg!(test) && url.scheme() == "http" && url.host_str() == Some("127.0.0.1");
    if (url.scheme() != "https" && !local_test)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || !matches!(
            url.path().trim_end_matches('/'),
            "" | "/speechtotext/transcriptions:transcribe"
        )
    {
        return Err(AppError::Config("Use the Azure Speech HTTPS resource endpoint without credentials, query, or extra path".into()));
    }
    url.set_path("/speechtotext/transcriptions:transcribe");
    url.query_pairs_mut()
        .append_pair("api-version", API_VERSION);
    Ok(url.into())
}

fn definition(language: Option<&str>) -> Result<Value, AppError> {
    let Some(language) = language
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "multi")
    else {
        return Ok(json!({}));
    };
    let locale = match language {
        "zh" => "zh-CN",
        "en" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "pt" => "pt-BR",
        "ru" => "ru-RU",
        "ar" => "ar-SA",
        "hi" => "hi-IN",
        "th" => "th-TH",
        "vi" => "vi-VN",
        "it" => "it-IT",
        "nl" => "nl-NL",
        "tr" => "tr-TR",
        "pl" => "pl-PL",
        "uk" => "uk-UA",
        "id" => "id-ID",
        "ms" => "ms-MY",
        locale if locale.contains('-') => locale,
        _ => {
            return Err(AppError::Config(format!(
                "Unsupported Azure Speech language: {language}"
            )))
        }
    };
    Ok(json!({"locales": [locale]}))
}

fn transcript(body: &Value) -> Result<Option<String>, AppError> {
    let phrases = body["combinedPhrases"].as_array().ok_or_else(|| {
        AppError::Config("Azure Speech response is missing combinedPhrases".into())
    })?;
    let mut texts = Vec::new();
    for phrase in phrases {
        let text = phrase["text"]
            .as_str()
            .ok_or_else(|| {
                AppError::Config("Azure Speech response contains an invalid transcript".into())
            })?
            .trim();
        if !text.is_empty() {
            texts.push(text);
        }
    }
    Ok((!texts.is_empty()).then(|| texts.join(" ")))
}

pub struct AzureSpeechProvider {
    client: reqwest::Client,
    config: Option<SttConfig>,
    endpoint: String,
    audio: Vec<u8>,
}

impl AzureSpeechProvider {
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            config: None,
            endpoint: String::new(),
            audio: Vec::new(),
        }
    }
}

#[async_trait]
impl SttProvider for AzureSpeechProvider {
    async fn connect(&mut self, config: &SttConfig) -> Result<(), AppError> {
        if config.api_key.trim().is_empty() {
            return Err(AppError::Auth("Azure Speech API key is empty".into()));
        }
        if config.sample_rate != 16_000 {
            return Err(AppError::Config(
                "Azure Speech requires 16 kHz mono PCM audio".into(),
            ));
        }
        self.endpoint =
            transcription_endpoint(config.provider_endpoint.as_deref().unwrap_or_default())?;
        definition(config.language.as_deref())?;
        self.config = Some(config.clone());
        self.audio.clear();
        Ok(())
    }

    async fn send_audio(&mut self, chunk: &[u8]) -> Result<(), AppError> {
        if self.config.is_none() {
            return Err(AppError::Config("Azure Speech is not connected".into()));
        }
        if self.audio.len().saturating_add(chunk.len()) > MAX_AUDIO_BYTES {
            return Err(AppError::Config(
                "Azure Speech audio exceeds the local buffer limit (~12 minutes)".into(),
            ));
        }
        self.audio.extend_from_slice(chunk);
        Ok(())
    }

    async fn recv_transcript(&mut self) -> Result<Option<TranscriptEvent>, AppError> {
        std::future::pending().await
    }

    async fn disconnect(&mut self) -> Result<Option<String>, AppError> {
        let Some(config) = self.config.take() else {
            return Ok(None);
        };
        if self.audio.is_empty() {
            return Ok(None);
        }
        let pcm = std::mem::take(&mut self.audio);
        let wav = WhisperCompatProvider::build_wav(&pcm, config.sample_rate);
        let definition = definition(config.language.as_deref())?.to_string();
        let started = std::time::Instant::now();
        let audio_ms = pcm.len() as u64 * 1000 / (u64::from(config.sample_rate) * 2);
        for attempt in 0..=MAX_RETRIES {
            let result = upload_once(
                &self.client,
                &self.endpoint,
                config.api_key.trim(),
                &wav,
                &definition,
            )
            .await;
            match result {
                Err(error) if error.is_retryable() && attempt < MAX_RETRIES => {
                    tracing::warn!(
                        "Azure Speech transient upload failure; retry {}/{}",
                        attempt + 1,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(Duration::from_secs(u64::from(attempt + 1))).await;
                }
                Err(AppError::AzureSpeechApi { status, details }) => {
                    return Err(AppError::AzureSpeechApi {
                        status,
                        details: format!(
                            "{details}\nAudio: {:.1}s; attempts: {}; elapsed: {:.1}s",
                            audio_ms as f64 / 1000.0,
                            attempt + 1,
                            started.elapsed().as_secs_f64(),
                        ),
                    });
                }
                result => return result,
            }
        }
        unreachable!("bounded retry loop always returns")
    }

    fn name(&self) -> &str {
        "Azure Speech"
    }
}

fn upload_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::Timeout(REQUEST_TIMEOUT)
    } else if error.is_decode() || error.is_builder() {
        AppError::Config(
            "Azure Speech returned an invalid response or request configuration".into(),
        )
    } else {
        error.into()
    }
}

// Only expose documented error fields, not arbitrary bodies, headers or URLs.
fn service_error_details(status: u16, body: &str, key: &str, request_id: Option<&str>) -> String {
    fn safe_text(value: &str, key: &str, limit: usize) -> String {
        let redacted = if key.is_empty() {
            value.to_string()
        } else {
            value.replace(key, "[redacted]")
        };
        redacted
            .split_whitespace()
            .map(|word| {
                if word.contains("://") || word.to_ascii_lowercase().starts_with("www.") {
                    "[url omitted]"
                } else {
                    word
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .filter(|c| !c.is_control())
            .take(limit)
            .collect()
    }

    let mut details = format!("Azure Speech — HTTP {status}");
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        let error = value.get("error").unwrap_or(&value);
        for field in ["code", "message"] {
            if let Some(text) = error.get(field).and_then(Value::as_str) {
                let text = safe_text(text, key, 360);
                if !text.is_empty() {
                    details.push_str(&format!("\n{field}: {text}"));
                }
            }
        }
    }
    if let Some(id) = request_id {
        let id = safe_text(id, key, 80);
        if !id.is_empty() {
            details.push_str(&format!("\nRequest ID: {id}"));
        }
    }
    details
}

async fn upload_once(
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
    wav: &[u8],
    definition: &str,
) -> Result<Option<String>, AppError> {
    let audio = reqwest::multipart::Part::bytes(wav.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(upload_error)?;
    let form = reqwest::multipart::Form::new()
        .part("audio", audio)
        .text("definition", definition.to_string());
    let response = client
        .post(endpoint)
        .header("Ocp-Apim-Subscription-Key", key)
        .multipart(form)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(upload_error)?;
    let status = response.status();
    if !status.is_success() {
        let request_id = ["apim-request-id", "x-ms-request-id"]
            .iter()
            .find_map(|name| response.headers().get(*name)?.to_str().ok())
            .map(str::to_string);
        // Preserve a permanent HTTP status even if reading its error body fails.
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Azure Speech error response body was unavailable".into());
        return Err(AppError::AzureSpeechApi {
            status: status.as_u16(),
            details: service_error_details(status.as_u16(), &body, key, request_id.as_deref()),
        });
    }
    let body: Value = response.json().await.map_err(upload_error)?;
    transcript(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azure_endpoint_requires_safe_resource_https_url() {
        assert_eq!(transcription_endpoint("https://speech.cognitiveservices.azure.com/").unwrap(),
            "https://speech.cognitiveservices.azure.com/speechtotext/transcriptions:transcribe?api-version=2025-10-15");
        for bad in [
            "",
            "http://example.com",
            "https://key@example.com",
            "https://example.com?key=x",
            "https://example.com/#fragment",
            "https://example.com/unrelated",
        ] {
            assert!(transcription_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn azure_locale_mapping_and_auto_detection() {
        assert_eq!(definition(None).unwrap(), json!({}));
        assert_eq!(definition(Some("multi")).unwrap(), json!({}));
        assert_eq!(
            definition(Some("zh")).unwrap(),
            json!({"locales": ["zh-CN"]})
        );
        assert_eq!(
            definition(Some("en-GB")).unwrap(),
            json!({"locales": ["en-GB"]})
        );
        assert!(definition(Some("unsupported")).is_err());
    }

    #[test]
    fn azure_preserves_display_transcripts_and_rejects_malformed_results() {
        assert_eq!(
            transcript(&json!({"combinedPhrases": [{"text": " 你好。 "}, {"text": "Hello!"}]}))
                .unwrap(),
            Some("你好。 Hello!".into())
        );
        assert_eq!(transcript(&json!({"combinedPhrases": []})).unwrap(), None);
        assert!(transcript(&json!({})).is_err());
        assert!(transcript(&json!({"combinedPhrases": [{}]})).is_err());
    }

    #[tokio::test]
    async fn azure_connection_and_buffer_are_bounded() {
        let mut provider = AzureSpeechProvider::with_client(reqwest::Client::new());
        assert!(provider.send_audio(&[0, 0]).await.is_err());
        let mut config = SttConfig {
            api_key: "key".into(),
            provider_endpoint: Some("https://speech.cognitiveservices.azure.com".into()),
            ..SttConfig::default()
        };
        provider.connect(&config).await.unwrap();
        assert!(provider
            .send_audio(&vec![0; MAX_AUDIO_BYTES + 1])
            .await
            .is_err());
        assert_eq!(provider.disconnect().await.unwrap(), None);
        config.api_key.clear();
        assert!(provider.connect(&config).await.is_err());
    }

    #[tokio::test]
    async fn azure_upload_retries_twice_with_identical_pcm_and_never_retries_permanent_errors() {
        use std::io::{Read, Write};
        for (statuses, expected_calls, success) in [
            (vec![503, 502, 200], 3, true),
            (vec![0, 0, 200], 3, true),
            (vec![503, 503, 503], 3, false),
            (vec![201], 1, false),
            (vec![400], 1, false),
            (vec![401], 1, false),
            (vec![429], 1, false),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let worker = std::thread::spawn(move || {
                let mut calls = 0;
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
                for status in statuses {
                    let mut socket = loop {
                        match listener.accept() {
                            Ok((socket, _)) => break socket,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                if std::time::Instant::now() > deadline {
                                    return calls;
                                }
                                std::thread::sleep(Duration::from_millis(5));
                            }
                            Err(error) => panic!("{error}"),
                        }
                    };
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut request = Vec::new();
                    loop {
                        let mut chunk = [0; 4096];
                        let count = socket.read(&mut chunk).unwrap();
                        assert!(count > 0);
                        request.extend_from_slice(&chunk[..count]);
                        if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                            let length: usize = headers
                                .lines()
                                .find_map(|line| line.strip_prefix("content-length: "))
                                .unwrap()
                                .parse()
                                .unwrap();
                            if request.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    let wav = WhisperCompatProvider::build_wav(&[17; 3200], 16000);
                    assert!(request.windows(wav.len()).any(|bytes| bytes == wav));
                    assert!(String::from_utf8_lossy(&request).contains("zh-CN"));
                    calls += 1;
                    if status == 0 {
                        drop(socket);
                        continue;
                    }
                    let body = if status == 200 {
                        r#"{"combinedPhrases":[{"text":"Recovered text."}]}"#
                    } else if status == 201 {
                        "not-json"
                    } else {
                        r#"{"error":{"message":"temporary or permanent test error"}}"#
                    };
                    write!(socket, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
                calls
            });
            let mut provider = AzureSpeechProvider::with_client(
                reqwest::Client::builder().no_proxy().build().unwrap(),
            );
            provider
                .connect(&SttConfig {
                    api_key: "test-key".into(),
                    language: Some("zh".into()),
                    provider_endpoint: Some(format!("http://{address}")),
                    ..Default::default()
                })
                .await
                .unwrap();
            provider.send_audio(&[17; 3200]).await.unwrap();
            let result = provider.disconnect().await;
            if success {
                assert_eq!(result.unwrap(), Some("Recovered text.".into()));
            } else {
                let error = result.unwrap_err();
                if let AppError::AzureSpeechApi { details, .. } = error {
                    assert!(details.contains(&format!("attempts: {expected_calls}")));
                    assert!(details.contains("Audio: 0.1s"));
                }
            }
            assert_eq!(worker.join().unwrap(), expected_calls);
        }
    }

    #[tokio::test]
    async fn cancelling_upload_backoff_prevents_further_attempts() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (responded_tx, responded_rx) = tokio::sync::oneshot::channel();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            socket
                .write_all(b"HTTP/1.1 503 Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
            drop(socket);
            responded_tx.send(()).unwrap();
            listener.set_nonblocking(true).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_millis(1300);
            while std::time::Instant::now() < deadline {
                assert!(
                    listener.accept().is_err(),
                    "Cancelled upload made another request"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let mut provider = AzureSpeechProvider::with_client(
            reqwest::Client::builder().no_proxy().build().unwrap(),
        );
        provider
            .connect(&SttConfig {
                api_key: "key".into(),
                provider_endpoint: Some(format!("http://{address}")),
                ..Default::default()
            })
            .await
            .unwrap();
        provider.send_audio(&[0; 3200]).await.unwrap();
        let cancellation = crate::stt_finalize::SttCancellation::default();
        let trigger = cancellation.clone();
        let upload = tokio::spawn(async move {
            tokio::select! { _ = cancellation.cancelled() => None, result = provider.disconnect() => Some(result) }
        });
        responded_rx.await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        trigger.cancel();
        assert!(upload.await.unwrap().is_none());
        worker.join().unwrap();
    }

    #[test]
    fn azure_service_diagnostics_are_structured_bounded_and_redacted() {
        let body = json!({"error": {
            "code": "TooManyRequests",
            "message": format!("Limit reached test-key https://host/?secret=hidden {}", "中".repeat(500)),
            "audio": "private audio content"
        }}).to_string();
        let details = service_error_details(429, &body, "test-key", Some("request-123"));
        assert!(details.contains("HTTP 429"));
        assert!(details.contains("TooManyRequests"));
        assert!(details.contains("[redacted]"));
        assert!(details.contains("Request ID: request-123"));
        assert!(!details.contains("test-key"));
        assert!(!details.contains("secret=hidden"));
        assert!(!details.contains("private audio"));
        assert!(details.chars().count() < 500);
        assert_eq!(
            service_error_details(502, "<html>private proxy response</html>", "key", None),
            "Azure Speech — HTTP 502"
        );
    }

    #[test]
    fn azure_http_diagnostics_preserve_status_mapping_and_retry_policy() {
        for status in [400, 401, 403, 429, 500, 503] {
            let error = AppError::AzureSpeechApi {
                status,
                details: format!("Azure Speech — HTTP {status}"),
            };
            assert_eq!(error.is_retryable(), status >= 500);
            let user = error.to_user_error();
            assert_eq!(
                user.code,
                if status == 401 || status == 403 {
                    "stt_invalid_key"
                } else {
                    "stt_failed"
                }
            );
            assert!(user.details.unwrap().contains(&status.to_string()));
        }
    }

    #[test]
    fn azure_finalization_covers_all_attempts_and_backoff() {
        assert!(FINALIZE_TIMEOUT >= REQUEST_TIMEOUT * 3 + Duration::from_secs(3));
    }

    #[tokio::test]
    async fn azure_upload_uses_speech_multipart_auth_and_reports_http_errors() {
        use std::io::{Read, Write};
        for (status, body) in [
            (
                "200 OK",
                r#"{"combinedPhrases":[{"text":"Hello, world."}]}"#,
            ),
            (
                "401 Unauthorized",
                r#"{"error":{"message":"Invalid subscription key"}}"#,
            ),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let worker = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut request = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&chunk[..n]);
                    if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                        let length: usize = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .unwrap()
                            .parse()
                            .unwrap();
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8_lossy(&request);
                assert!(request
                    .contains("/speechtotext/transcriptions:transcribe?api-version=2025-10-15"));
                assert!(request
                    .to_lowercase()
                    .contains("ocp-apim-subscription-key: test-key"));
                assert!(!request.to_lowercase().contains("authorization:"));
                assert!(request.contains("name=\"audio\""));
                assert!(request.contains("RIFF"));
                assert!(request.contains("name=\"definition\""));
                assert!(request.contains("zh-CN"));
                write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\napim-request-id: fixture-request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            });
            let mut provider = AzureSpeechProvider::with_client(
                reqwest::Client::builder().no_proxy().build().unwrap(),
            );
            provider
                .connect(&SttConfig {
                    api_key: "test-key".into(),
                    language: Some("zh".into()),
                    provider_endpoint: Some(format!("http://{address}")),
                    ..SttConfig::default()
                })
                .await
                .unwrap();
            provider.send_audio(&[0; 3200]).await.unwrap();
            let result = provider.disconnect().await;
            if status.starts_with("200") {
                assert_eq!(result.unwrap(), Some("Hello, world.".into()));
            } else {
                let error = result.unwrap_err();
                assert!(matches!(
                    error,
                    AppError::AzureSpeechApi { status: 401, .. }
                ));
                let details = error.to_user_error().details.unwrap();
                assert!(details.contains("HTTP 401"));
                assert!(details.contains("Invalid subscription key"));
                assert!(details.contains("Request ID: fixture-request"));
                assert!(details.contains("attempts: 1"));
            }
            worker.join().unwrap();
        }
    }
}
