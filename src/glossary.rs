use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_GLOSSARY_PATH: &str = "glossary.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct GlossaryStore {
    path: PathBuf,
    entries: Vec<GlossaryEntry>,
}

impl Default for GlossaryStore {
    fn default() -> Self {
        Self::load(DEFAULT_GLOSSARY_PATH)
    }
}

impl GlossaryStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<GlossaryEntry>>(&s).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    pub fn entries(&self) -> &[GlossaryEntry] {
        &self.entries
    }

    pub fn add_or_update(&mut self, source: &str, target: &str) -> bool {
        let src = source.trim();
        let tgt = target.trim();
        if src.is_empty() || tgt.is_empty() {
            return false;
        }
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.source.eq_ignore_ascii_case(src))
        {
            existing.target = tgt.to_string();
        } else {
            self.entries.push(GlossaryEntry {
                source: src.to_string(),
                target: tgt.to_string(),
            });
        }
        self.save();
        true
    }

    pub fn remove_at(&mut self, index: usize) -> bool {
        if index < self.entries.len() {
            self.entries.remove(index);
            self.save();
            true
        } else {
            false
        }
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            let _ = fs::write(&self.path, json);
        }
    }

    /// Formats the glossary as a concise instruction block for AI system prompts.
    pub fn format_prompt_section(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = self
            .entries
            .iter()
            .map(|e| format!("- \"{}\" => \"{}\"", e.source, e.target))
            .collect();
        format!(
            "\n\nUse these exact glossary translations when the terms appear:\n{}",
            lines.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn crud_and_prompt_formatting_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("glossary.json");
        let mut store = GlossaryStore::load(&path);

        assert!(store.format_prompt_section().is_empty());
        assert!(!store.add_or_update("  ", "foo"));
        assert!(store.add_or_update("Antigravity", "Antigravity"));
        assert!(store.add_or_update("พินโต", "Pinto"));
        // Update existing entry case-insensitively
        assert!(store.add_or_update("antigravity", "AGY"));
        assert_eq!(store.entries().len(), 2);

        let prompt = store.format_prompt_section();
        assert!(prompt.contains("\"Antigravity\" => \"AGY\""));
        assert!(prompt.contains("\"พินโต\" => \"Pinto\""));

        // Reload from disk
        let reloaded = GlossaryStore::load(&path);
        assert_eq!(reloaded.entries().len(), 2);
        assert_eq!(reloaded.entries()[0].target, "AGY");

        let mut mutable = reloaded;
        assert!(mutable.remove_at(0));
        assert_eq!(mutable.entries().len(), 1);
    }
}
