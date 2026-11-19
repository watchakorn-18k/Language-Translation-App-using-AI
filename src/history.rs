use crate::glossary::csv_escape;
use crate::paths;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_HISTORY_PATH: &str = "translation_history.json";
pub const MAX_HISTORY_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: u64,
    pub timestamp: String,
    pub provider: String,
    pub source_text: String,
    pub primary_english: String,
    pub target_language: String,
    pub secondary_localized: String,
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
    entries: Vec<HistoryEntry>,
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self::load(paths::config_file(DEFAULT_HISTORY_PATH))
    }
}

impl HistoryStore {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<HistoryEntry>>(&s).ok())
            .unwrap_or_default();
        Self { path, entries }
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn push(
        &mut self,
        provider: &str,
        source_text: &str,
        primary_english: &str,
        target_language: &str,
        secondary_localized: &str,
    ) {
        if source_text.trim().is_empty() && primary_english.trim().is_empty() {
            return;
        }
        let next_id = self.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let entry = HistoryEntry {
            id: next_id,
            timestamp: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            provider: provider.to_string(),
            source_text: source_text.trim().to_string(),
            primary_english: primary_english.trim().to_string(),
            target_language: target_language.to_string(),
            secondary_localized: secondary_localized.trim().to_string(),
        };
        self.entries.insert(0, entry);
        if self.entries.len() > MAX_HISTORY_ENTRIES {
            self.entries.truncate(MAX_HISTORY_ENTRIES);
        }
        self.save();
    }

    pub fn search(&self, query: &str) -> Vec<HistoryEntry> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return self.entries.clone();
        }
        self.entries
            .iter()
            .filter(|e| {
                e.source_text.to_ascii_lowercase().contains(&q)
                    || e.primary_english.to_ascii_lowercase().contains(&q)
                    || e.secondary_localized.to_ascii_lowercase().contains(&q)
                    || e.provider.to_ascii_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn remove_by_id(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
        self.save();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.save();
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            let _ = fs::write(&self.path, json);
        }
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::from("timestamp,provider,source,english,target_language,target\n");
        for e in &self.entries {
            let _ = writeln!(
                out,
                "{},{},{},{},{},{}",
                csv_escape(&e.timestamp),
                csv_escape(&e.provider),
                csv_escape(&e.source_text),
                csv_escape(&e.primary_english),
                csv_escape(&e.target_language),
                csv_escape(&e.secondary_localized)
            );
        }
        out
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# Translation History\n\n");
        for e in &self.entries {
            let _ = writeln!(out, "## {} ({})\n", e.timestamp, e.provider);
            let _ = writeln!(out, "- **Source:** {}", e.source_text);
            let _ = writeln!(out, "- **English:** {}", e.primary_english);
            let _ = writeln!(
                out,
                "- **{}:** {}\n",
                e.target_language, e.secondary_localized
            );
        }
        out
    }

    /// Exports history as `.md` (Markdown) or `.csv` based on the file extension.
    pub fn export_to(&self, path: &Path) -> io::Result<()> {
        let is_md = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"));
        if is_md {
            fs::write(path, self.to_markdown())
        } else {
            fs::write(path, self.to_csv())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn stores_newest_first_searches_and_persists() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut store = HistoryStore::load(&path);

        store.push("OpenAI", "สวัสดี", "Hello", "thai", "สวัสดี");
        store.push("Gemini", "ขอบคุณ", "Thank you", "thai", "ขอบคุณ");

        assert_eq!(store.entries().len(), 2);
        assert_eq!(store.entries()[0].primary_english, "Thank you");
        assert_eq!(store.entries()[1].primary_english, "Hello");

        let hits = store.search("hello");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].provider, "OpenAI");

        let reloaded = HistoryStore::load(&path);
        assert_eq!(reloaded.entries().len(), 2);
    }

    #[test]
    fn enforces_max_history_cap() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history_cap.json");
        let mut store = HistoryStore::load(&path);

        for i in 0..(MAX_HISTORY_ENTRIES + 15) {
            store.push(
                "Google Translate",
                &format!("src {i}"),
                &format!("en {i}"),
                "thai",
                "th",
            );
        }
        assert_eq!(store.entries().len(), MAX_HISTORY_ENTRIES);
    }

    #[test]
    fn exports_history_to_csv_and_markdown() {
        let dir = tempdir().unwrap();
        let mut store = HistoryStore::load(dir.path().join("h.json"));
        store.push("OpenAI", "Hello, world", "Hello, world", "thai", "สวัสดี โลก");

        let csv_path = dir.path().join("history.csv");
        let md_path = dir.path().join("history.md");
        store.export_to(&csv_path).unwrap();
        store.export_to(&md_path).unwrap();

        let csv = fs::read_to_string(&csv_path).unwrap();
        assert!(csv.starts_with("timestamp,provider,source,english,target_language,target\n"));
        assert!(csv.contains("\"Hello, world\""));

        let md = fs::read_to_string(&md_path).unwrap();
        assert!(md.starts_with("# Translation History"));
        assert!(md.contains("**English:** Hello, world"));
        assert!(md.contains("**thai:** สวัสดี โลก"));
    }
}
