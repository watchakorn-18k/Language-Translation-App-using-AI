use serde_json::Value;

/// Trim leading and trailing whitespace (matches `re.sub(r"^\s+|\s+$", "", response)` in `app.py`).
pub fn trim_whitespace(input: &str) -> String {
    input.trim().to_string()
}

/// Safely extract `{"translation_result": "..."}` from model output without risking
/// UTF-8 byte-boundary panics (fixes the byte/char offset risk from `Bard.py` and `Bingchat.py`).
pub fn extract_translation_result(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end < start {
        return None;
    }
    let slice = &raw[start..=end];
    let parsed: Value = serde_json::from_str(slice).ok()?;
    parsed
        .get("translation_result")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_embedded_json_with_unicode() {
        let raw = "ผลลัพธ์: ```json\n{\"translation_result\": \"Hello world\"}\n```";
        assert_eq!(
            extract_translation_result(raw).as_deref(),
            Some("Hello world")
        );
    }

    #[test]
    fn returns_none_on_invalid_json() {
        assert_eq!(extract_translation_result("no braces here"), None);
        assert_eq!(extract_translation_result("{invalid}"), None);
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        assert_eq!(trim_whitespace("  \n\tสวัสดี \r\n "), "สวัสดี");
    }
}
