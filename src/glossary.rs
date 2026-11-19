use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::paths;

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
        Self::load(paths::config_file(DEFAULT_GLOSSARY_PATH))
    }
}

/// Escapes one CSV field (RFC 4180): quotes fields containing commas, quotes or newlines.
pub fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Parses CSV text into rows of fields (handles quoted fields, escaped quotes and embedded newlines).
pub fn parse_csv(input: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();
    let mut row_has_content = false;

    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' => {
                in_quotes = true;
                row_has_content = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                row_has_content = true;
            }
            '\r' => {}
            '\n' => {
                if row_has_content || !field.is_empty() {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                row_has_content = false;
            }
            _ => {
                field.push(c);
                row_has_content = true;
            }
        }
    }
    if row_has_content || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
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
            .find(|e| e.source.to_lowercase() == src.to_lowercase())
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

    /// Exports as CSV with a `source,target` header.
    pub fn to_csv(&self) -> String {
        let mut out = String::from("source,target\n");
        for e in &self.entries {
            let _ = writeln!(out, "{},{}", csv_escape(&e.source), csv_escape(&e.target));
        }
        out
    }

    /// Merges terms from CSV text (`source,target`, header optional). Returns how many were added/updated.
    pub fn import_csv(&mut self, csv: &str) -> usize {
        let mut count = 0;
        for (i, row) in parse_csv(csv).iter().enumerate() {
            if row.len() < 2 {
                continue;
            }
            if i == 0
                && row[0].trim().eq_ignore_ascii_case("source")
                && row[1].trim().eq_ignore_ascii_case("target")
            {
                continue;
            }
            if self.add_or_update(&row[0], &row[1]) {
                count += 1;
            }
        }
        count
    }

    /// Merges terms from a JSON array of `{source, target}` objects.
    pub fn import_json(&mut self, json: &str) -> usize {
        let Ok(entries) = serde_json::from_str::<Vec<GlossaryEntry>>(json) else {
            return 0;
        };
        entries
            .iter()
            .filter(|e| self.add_or_update(&e.source, &e.target))
            .count()
    }

    pub fn export_to(&self, path: &Path) -> io::Result<()> {
        let is_json = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if is_json {
            let json = serde_json::to_string_pretty(&self.entries).map_err(io::Error::other)?;
            fs::write(path, json)
        } else {
            fs::write(path, self.to_csv())
        }
    }

    /// Imports a `.json` or `.csv` file. Returns the number of terms added or updated.
    pub fn import_from(&mut self, path: &Path) -> io::Result<usize> {
        let content = fs::read_to_string(path)?;
        let is_json = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        Ok(if is_json {
            self.import_json(&content)
        } else {
            self.import_csv(&content)
        })
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
        assert!(!store.add_or_update("foo", "  "));
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
        assert!(!mutable.remove_at(5));
        assert_eq!(mutable.entries().len(), 1);
    }

    #[test]
    fn csv_escape_quotes_only_when_needed() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn csv_parser_handles_quotes_commas_newlines_bom_and_crlf() {
        let csv = "\u{feff}source,target\r\n\"a,b\",\"say \"\"hi\"\"\"\r\n\"multi\nline\",ok\r\n\r\nlast,row";
        let rows = parse_csv(csv);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], vec!["source", "target"]);
        assert_eq!(rows[1], vec!["a,b", "say \"hi\""]);
        assert_eq!(rows[2], vec!["multi\nline", "ok"]);
        assert_eq!(rows[3], vec!["last", "row"]);
        assert!(parse_csv("").is_empty());
        assert!(parse_csv("\n\n").is_empty());
    }

    #[test]
    fn csv_export_import_roundtrip_with_special_characters() {
        let dir = tempdir().unwrap();
        let mut a = GlossaryStore::load(dir.path().join("a.json"));
        a.add_or_update("Hello, world", "สวัสดี \"โลก\"");
        a.add_or_update("multi\nline", "บรรทัด");
        a.add_or_update("CEO", "ซีอีโอ");

        let csv = a.to_csv();
        assert!(csv.starts_with("source,target\n"));

        let mut b = GlossaryStore::load(dir.path().join("b.json"));
        assert_eq!(b.import_csv(&csv), 3);
        assert_eq!(b.entries(), a.entries());
        // Importing again updates instead of duplicating.
        assert_eq!(b.import_csv(&csv), 3);
        assert_eq!(b.entries().len(), 3);
    }

    #[test]
    fn import_skips_header_blank_and_short_rows() {
        let dir = tempdir().unwrap();
        let mut g = GlossaryStore::load(dir.path().join("g.json"));
        let n = g.import_csv("Source,Target\nonly-one-column\n,\nreal,term\n");
        assert_eq!(n, 1);
        assert_eq!(g.entries().len(), 1);
        assert_eq!(g.entries()[0].source, "real");
    }

    #[test]
    fn file_export_import_picks_format_by_extension() {
        let dir = tempdir().unwrap();
        let mut g = GlossaryStore::load(dir.path().join("g.json"));
        g.add_or_update("AI", "เอไอ");

        let json_path = dir.path().join("out.json");
        let csv_path = dir.path().join("out.csv");
        g.export_to(&json_path).unwrap();
        g.export_to(&csv_path).unwrap();
        assert!(
            fs::read_to_string(&json_path)
                .unwrap()
                .trim_start()
                .starts_with('[')
        );
        assert!(
            fs::read_to_string(&csv_path)
                .unwrap()
                .starts_with("source,target")
        );

        let mut from_json = GlossaryStore::load(dir.path().join("j.json"));
        assert_eq!(from_json.import_from(&json_path).unwrap(), 1);
        let mut from_csv = GlossaryStore::load(dir.path().join("c.json"));
        assert_eq!(from_csv.import_from(&csv_path).unwrap(), 1);
        assert_eq!(from_json.entries(), from_csv.entries());

        assert!(
            from_csv
                .import_from(&dir.path().join("missing.csv"))
                .is_err()
        );
        assert_eq!(from_csv.import_json("not json"), 0);
    }
}
