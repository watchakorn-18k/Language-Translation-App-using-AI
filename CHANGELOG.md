# Changelog

All notable changes to **Language Translation App with AI** are documented in this file.

## [7.0.0] - 2026-11-20

### Added (Phases 4–7)
- **Fixed macOS Thai / Multilingual OCR:** Prioritized Apple Vision (`VNRecognizeTextRequest`) on macOS and added `tesseract --list-langs` verification so missing language packs never silently output English-only garbage.
- **Expanded OCR Languages (9 scripts):** Auto-detect, Thai, Chinese (Simplified & Traditional), Japanese, Korean, Turkish, Arabic, and Vietnamese.
- **Built-in Offline OCR (`ocrs` + `rten`):** Pure-Rust ONNX OCR fallback (`src/builtin_ocr.rs`) using models stored in `<data_dir>/models`.
- **Real Runtime Always-on-Top (`src/window_level.rs`):** Instant native window-level toggling on macOS (`NSWindow.setLevel`) and Windows (`SetWindowPos`).
- **Floating Translation HUD Overlay (`src/translation_overlay.rs`):** Optional floating popup after OCR capture for games, manga, and fullscreen reading.
- **Clipboard Watcher (`src/clipboard_watch.rs`):** Optional automatic translation when new text is copied to the system clipboard (ignores self-copies).
- **Voice Text-to-Speech (`src/tts.rs`):** Speak English or target-language translations aloud using native OS voices (`Volume2` icon on both tiers).
- **Tabbed Settings & Theme Switcher (`src/theme.rs`):** Split Settings into **Engine** and **Behavior** tabs and added **System / Dark / Light** Graphite palettes.
- **History & Glossary Import/Export:** Export translation history to CSV or Markdown, and import/export custom glossary files (`.csv` / `.json`).
- **In-App Update Downloader (`src/updater.rs`):** Automatically selects and downloads the matching platform release asset into `Downloads` and reveals it in Finder/Explorer.
- **OS User Data Directory (`src/paths.rs`):** Moved all config, history, glossary, and cache files into the platform standard user data directory (`~/Library/Application Support/language-translation-app` on macOS, `%APPDATA%\language-translation-app` on Windows, `~/.config/language-translation-app` on Linux) with automatic one-time migration from the working directory.
- **Native macOS `.icns` Icon & Pure-Rust Repository:** Added `src/icon.icns`, `src/icon.png`, Linux `.desktop` entry, and removed legacy Python files.

## [6.3.0] - 2026-11-20

### Added (Phases 1–3)
- OS-wide global hotkey (`global-hotkey`) for `⌘⇧Z` / `Ctrl+Shift+Z`
- Auto-translate after OCR, Single-Hop Direct AI Translation, Vision AI OCR, and live SSE streaming
- Multi-monitor drag-capture, Translation History, Custom Glossary, and OS Keychain (`keyring`)
- Windows native WinRT offline OCR, embedded Windows `.exe` icon (`build.rs`), and macOS `.app` + `.dmg` CI packaging

## [6.0.0] - 2026-11-20

### Changed
- Rewrote the entire application in Rust using `gpui-kit 0.10.0`
- Redesigned UI following `unslop` principles (Graphite dark ramp, bilingual two-tier sheet, ISO tags, keycaps)
- Upgraded AI providers to 2026 frontier models (`gpt-6.2-sol`, `gemini-4-neon`, `claude-sonnet-5-5`, `deepseek-chat`, `gemma4`)
