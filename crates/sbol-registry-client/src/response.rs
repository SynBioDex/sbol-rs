//! Buffered HTTP response data and registry error decoding.

pub(crate) struct BufferedResponse {
    pub(crate) body: Vec<u8>,
    pub(crate) content_type: Option<String>,
    pub(crate) etag: Option<String>,
}

pub(crate) fn error_message(body: &[u8]) -> String {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        for pointer in ["/error/message", "/message", "/error"] {
            if let Some(message) = value.pointer(pointer).and_then(|value| value.as_str()) {
                return message.to_owned();
            }
        }
    }
    let text = String::from_utf8_lossy(body);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "empty response body".to_owned()
    } else {
        trimmed.chars().take(500).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_structured_error_message() {
        let body = br#"{"error":{"code":"not_found","message":"design missing"}}"#;
        assert_eq!(error_message(body), "design missing");
    }
}
