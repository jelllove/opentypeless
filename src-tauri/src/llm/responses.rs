//! Stateless Azure Responses wire format, separate from Chat Completions.
use serde_json::{json, Value};

use super::protocol::StreamEvent;

pub fn request_body(model: &str, messages: Vec<Value>, max_tokens: u32, stream: bool) -> Value {
    json!({
        "model": model,
        "input": messages,
        "max_output_tokens": max_tokens,
        "store": false,
        "stream": stream
    })
}

pub fn text(body: &Value) -> String {
    body["output"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["type"].as_str() == Some("message"))
        .flat_map(|item| item["content"].as_array().into_iter().flatten())
        .filter(|part| part["type"].as_str() == Some("output_text"))
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

pub fn response_error(body: &Value) -> Option<String> {
    if !body["error"].is_null() {
        return Some(
            body["error"]["message"]
                .as_str()
                .unwrap_or("Azure Responses request failed")
                .chars()
                .take(300)
                .collect(),
        );
    }
    match body["status"].as_str() {
        Some("failed" | "cancelled" | "incomplete") => Some(format!(
            "Azure Responses did not complete ({}): {}",
            body["status"].as_str().unwrap_or("unknown"),
            body["incomplete_details"]["reason"]
                .as_str()
                .unwrap_or("No completed output")
                .chars()
                .take(200)
                .collect::<String>()
        )),
        _ => None,
    }
}

pub fn stream_event(body: &Value) -> StreamEvent {
    match body["type"].as_str() {
        Some("response.output_text.delta") => StreamEvent {
            text: body["delta"].as_str().map(str::to_string),
            ..Default::default()
        },
        Some("response.completed") => StreamEvent {
            done: true,
            error: response_error(&body["response"]),
            ..Default::default()
        },
        Some("response.failed" | "response.incomplete" | "response.cancelled") => StreamEvent {
            done: true,
            error: Some(response_error(&body["response"]).unwrap_or_else(|| {
                format!(
                    "Azure Responses stream ended with {}",
                    body["type"].as_str().unwrap_or("failure")
                )
            })),
            ..Default::default()
        },
        Some("error") => StreamEvent {
            done: true,
            error: Some(
                body["message"]
                    .as_str()
                    .or_else(|| body["error"]["message"].as_str())
                    .unwrap_or("Azure Responses stream failed")
                    .chars()
                    .take(300)
                    .collect(),
            ),
            ..Default::default()
        },
        _ => StreamEvent::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn azure_responses_never_treats_incomplete_output_as_success() {
        let body = json!({"status": "incomplete", "incomplete_details": {"reason": "max_output_tokens"}, "output": [{"type": "message", "content": [{"type": "output_text", "text": "Partial answer"}]}]});
        assert!(response_error(&body).unwrap().contains("max_output_tokens"));
        assert!(
            stream_event(&json!({"type": "response.incomplete", "response": body}))
                .error
                .is_some()
        );
        assert_eq!(
            text(
                &json!({"output": [{"type": "reasoning", "summary": []}, {"type": "message", "content": [{"type": "output_text", "text": "你好"}, {"type": "output_text", "text": "。"}]}]})
            ),
            "你好。"
        );
    }
}
