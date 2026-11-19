/// Tracks system clipboard text changes while ignoring self-generated copies
/// (e.g. when the user clicks Copy inside the translator or OCR writes to clipboard).
#[derive(Debug, Clone, Default)]
pub struct ClipboardWatcher {
    last_seen: String,
    ignored_self_copy: Option<String>,
}

impl ClipboardWatcher {
    pub fn new(initial: String) -> Self {
        Self {
            last_seen: initial,
            ignored_self_copy: None,
        }
    }

    /// Marks text copied by the app itself so the next poll does not re-trigger translation.
    pub fn mark_self_copy(&mut self, text: &str) {
        let t = text.trim().to_string();
        self.last_seen = t.clone();
        self.ignored_self_copy = Some(t);
    }

    /// Evaluates newly polled clipboard text. Returns `Some(text)` only if it is new,
    /// non-empty, reasonably sized, and not a self-copy.
    pub fn poll(&mut self, current: &str) -> Option<String> {
        let trimmed = current.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 4000 {
            return None;
        }
        if trimmed == self.last_seen {
            return None;
        }
        if self.ignored_self_copy.as_deref() == Some(trimmed) {
            self.last_seen = trimmed.to_string();
            return None;
        }
        self.ignored_self_copy = None;
        self.last_seen = trimmed.to_string();
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_new_clipboard_text_and_ignores_duplicates() {
        let mut w = ClipboardWatcher::new("initial".into());
        assert_eq!(w.poll("initial"), None);
        assert_eq!(w.poll("  "), None);
        assert_eq!(w.poll("สวัสดี").as_deref(), Some("สวัสดี"));
        assert_eq!(w.poll("สวัสดี"), None);
    }

    #[test]
    fn ignores_app_self_copies() {
        let mut w = ClipboardWatcher::new("old".into());
        w.mark_self_copy("Translated output");
        assert_eq!(w.poll("Translated output"), None);
        assert_eq!(w.poll("New user copy").as_deref(), Some("New user copy"));
    }

    #[test]
    fn skips_oversized_clipboard_payloads() {
        let mut w = ClipboardWatcher::default();
        let huge = "a".repeat(5000);
        assert_eq!(w.poll(&huge), None);
    }
}
