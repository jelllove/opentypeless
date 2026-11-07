use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use super::{whisper_compat::WhisperCompatProvider, SttConfig, SttProvider, TranscriptEvent};
use crate::error::AppError;

pub const PROVIDER: &str = "azure-speech";
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
pub const FINALIZE_TIMEOUT: Duration = Duration::from_secs(135);
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
        let audio = reqwest::multipart::Part::bytes(WhisperCompatProvider::build_wav(
            &pcm,
            config.sample_rate,
        ))
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(AppError::from)?;
        let form = reqwest::multipart::Form::new().part("audio", audio).text(
            "definition",
            definition(config.language.as_deref())?.to_string(),
        );
        let response = self
            .client
            .post(&self.endpoint)
            .header("Ocp-Apim-Subscription-Key", config.api_key.trim())
            .multipart(form)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AppError::Timeout(REQUEST_TIMEOUT)
                } else {
                    e.into()
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            let body: String = response.text().await?.chars().take(300).collect();
            return Err(AppError::Api {
                status: status.as_u16(),
                body,
            });
        }
        transcript(&response.json::<Value>().await?)
    }

    fn name(&self) -> &str {
        "Azure Speech"
    }
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
                write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
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
                assert!(matches!(result, Err(AppError::Api { status: 401, .. })));
            }
            worker.join().unwrap();
        }
    }
}
