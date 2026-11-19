use std::fs;
use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "language-translation-app";
/// Overrides the user data directory (used by tests, demos and portable installs).
pub const DATA_DIR_ENV: &str = "TRANSLATE_APP_DATA_DIR";

/// Config files that older versions (<= 6.x) wrote into the current working directory.
pub const LEGACY_CONFIG_FILES: &[&str] = &[
    "app_settings.json",
    "api_key_chatgpt.json",
    "api_gemini.json",
    "api_claude.json",
    "api_deepseek.json",
    "ollama_config.json",
    "custom_openai.json",
    "glossary.json",
    "translation_history.json",
];

/// Pure resolution logic: an explicit override wins, otherwise `<config dir>/<app name>`.
pub fn resolve_data_dir(env_override: Option<&str>, config_base: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = env_override.map(str::trim).filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    config_base
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

/// Per-user data directory:
/// - macOS: `~/Library/Application Support/language-translation-app`
/// - Windows: `%APPDATA%\language-translation-app`
/// - Linux: `~/.config/language-translation-app`
pub fn data_dir() -> PathBuf {
    let dir = resolve_data_dir(
        std::env::var(DATA_DIR_ENV).ok().as_deref(),
        dirs::config_dir(),
    );
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn config_file(name: &str) -> PathBuf {
    data_dir().join(name)
}

pub fn cache_dir() -> PathBuf {
    let dir = data_dir().join("cache");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn models_dir() -> PathBuf {
    let dir = data_dir().join("models");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn downloads_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Moves each named file from `from` into `to` unless `to` already has a file with that name.
/// Returns the names that were migrated.
pub fn migrate_files(from: &Path, to: &Path, names: &[&str]) -> Vec<String> {
    let mut moved = Vec::new();
    if same_dir(from, to) {
        return moved;
    }
    let _ = fs::create_dir_all(to);
    for name in names {
        let src = from.join(name);
        let dst = to.join(name);
        if !src.is_file() || dst.exists() {
            continue;
        }
        let ok = fs::rename(&src, &dst).is_ok()
            || (fs::copy(&src, &dst).is_ok() && fs::remove_file(&src).is_ok());
        if ok {
            moved.push((*name).to_string());
        }
    }
    moved
}

/// One-time migration of config files from the working directory (pre-7.0 layout).
pub fn migrate_legacy_files() -> Vec<String> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    migrate_files(&cwd, &data_dir(), LEGACY_CONFIG_FILES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn override_wins_over_platform_dir() {
        let dir = resolve_data_dir(Some("/tmp/custom"), Some(PathBuf::from("/base")));
        assert_eq!(dir, PathBuf::from("/tmp/custom"));
    }

    #[test]
    fn blank_override_falls_back_to_platform_dir() {
        let dir = resolve_data_dir(Some("   "), Some(PathBuf::from("/base")));
        assert_eq!(dir, PathBuf::from("/base").join(APP_DIR_NAME));
    }

    #[test]
    fn missing_platform_dir_uses_current_directory() {
        let dir = resolve_data_dir(None, None);
        assert_eq!(dir, PathBuf::from(".").join(APP_DIR_NAME));
    }

    #[test]
    fn migration_moves_only_missing_files_and_never_overwrites() {
        let from = tempdir().unwrap();
        let to = tempdir().unwrap();
        fs::write(from.path().join("app_settings.json"), "old-settings").unwrap();
        fs::write(from.path().join("glossary.json"), "old-glossary").unwrap();
        fs::write(from.path().join("unrelated.txt"), "keep").unwrap();
        fs::write(to.path().join("glossary.json"), "new-glossary").unwrap();

        let moved = migrate_files(from.path(), to.path(), LEGACY_CONFIG_FILES);

        assert_eq!(moved, vec!["app_settings.json".to_string()]);
        assert_eq!(
            fs::read_to_string(to.path().join("app_settings.json")).unwrap(),
            "old-settings"
        );
        // Existing destination file is untouched and the source copy is left alone.
        assert_eq!(
            fs::read_to_string(to.path().join("glossary.json")).unwrap(),
            "new-glossary"
        );
        assert!(from.path().join("glossary.json").exists());
        // Files outside the legacy list are never touched.
        assert!(from.path().join("unrelated.txt").exists());
        assert!(!to.path().join("unrelated.txt").exists());
    }

    #[test]
    fn migration_is_a_noop_for_the_same_directory() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("app_settings.json"), "x").unwrap();
        assert!(migrate_files(dir.path(), dir.path(), LEGACY_CONFIG_FILES).is_empty());
        assert!(dir.path().join("app_settings.json").exists());
    }
}
