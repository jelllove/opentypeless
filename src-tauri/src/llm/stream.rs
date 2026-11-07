use super::{
    protocol::{self, LlmApiKind},
    ChunkCallback,
};
use crate::error::AppError;
use futures_util::StreamExt;

pub async fn read_text(
    response: reqwest::Response,
    kind: LlmApiKind,
    callback: &ChunkCallback,
) -> Result<String, AppError> {
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut done = false;
    while !done {
        let Some(chunk) = stream.next().await else {
            break;
        };
        buffer.extend_from_slice(&chunk?);
        while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            let line_bytes: Vec<u8> = buffer.drain(..=end).collect();
            let line = std::str::from_utf8(&line_bytes)
                .map_err(|_| AppError::Config("LLM stream contains invalid UTF-8".into()))?
                .trim();
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                if kind != LlmApiKind::OpenAiResponses {
                    done = true;
                    break;
                }
                continue;
            }
            let Ok(body) = serde_json::from_str(data) else {
                continue;
            };
            let event = protocol::parse_stream_event(kind, &body);
            if let Some(error) = event.error {
                return Err(AppError::Config(error));
            }
            if let Some(part) = event.text {
                if !part.is_empty() {
                    text.push_str(&part);
                    callback(&part);
                }
            }
            if let Some(part) = event.reasoning {
                reasoning.push_str(&part);
            }
            if event.done {
                done = true;
                break;
            }
        }
    }
    if kind == LlmApiKind::OpenAiResponses && !done {
        return Err(AppError::Config(
            "Azure Responses stream ended before completion".into(),
        ));
    }
    if text.is_empty() && !reasoning.is_empty() {
        callback(&reasoning);
        text = reasoning;
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn response(chunks: Vec<Vec<u8>>) -> reqwest::Response {
        let stream = futures_util::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>));
        http::Response::builder()
            .status(200)
            .body(reqwest::Body::wrap_stream(stream))
            .unwrap()
            .into()
    }

    #[tokio::test]
    async fn responses_stream_handles_split_utf8_and_requires_completed_event() {
        let payload = "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"你好。\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n";
        let chunks = payload
            .as_bytes()
            .chunks(1)
            .map(|part| part.to_vec())
            .collect();
        let callback: ChunkCallback = Box::new(|_| {});
        assert_eq!(
            read_text(response(chunks), LlmApiKind::OpenAiResponses, &callback)
                .await
                .unwrap(),
            "你好。"
        );
        for end in [
            "",
            "data: [DONE]\n\n",
            "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\"}}\n\n",
            "data: {\"type\":\"error\",\"message\":\"quota exceeded\"}\n\n",
        ] {
            let payload = format!(
                "data: {{\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}}\n\n{end}"
            );
            assert!(read_text(
                response(vec![payload.into_bytes()]),
                LlmApiKind::OpenAiResponses,
                &callback
            )
            .await
            .is_err());
        }
    }

    #[tokio::test]
    async fn chat_completions_stream_keeps_content_and_reasoning_fallback() {
        let callback: ChunkCallback = Box::new(|_| {});
        for (delta, expected) in [("content", "Hello"), ("reasoning_content", "Hello")] {
            let payload = format!("data: {{\"choices\":[{{\"delta\":{{\"{delta}\":\"Hello\"}}}}]}}\n\ndata: [DONE]\n\n");
            assert_eq!(
                read_text(
                    response(vec![payload.into_bytes()]),
                    LlmApiKind::OpenAiCompatible,
                    &callback
                )
                .await
                .unwrap(),
                expected
            );
        }
    }
}
