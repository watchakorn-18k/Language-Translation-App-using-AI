# Contributing to Language Translation App with AI

Thank you for contributing! This project is written in **100% Rust** using **[gpui-kit](https://github.com/longbridge/gpui-kit)**.

## Development Setup

1. Install the stable Rust toolchain via [rustup](https://rustup.rs/).
2. Clone the repository and run the test suite:
   ```bash
   cargo test --lib
   ```
3. Launch the desktop app in development mode:
   ```bash
   cargo run
   ```

## Code Quality Checks

Before opening a pull request, make sure all checks pass without warnings:

```bash
cargo fmt --check
cargo clippy --lib --bins -- -D warnings
cargo test --lib
```

## Architecture Overview

- `src/main.rs` — Application entrypoint, global hotkey registration, theme & window initialization
- `src/ui.rs` — Main GPUI view (`TranslateApp`), Bilingual Translation Sheet, and dialogs (Settings, History, Glossary, Target Language)
- `src/snipper.rs` & `src/snipper_overlay.rs` — Multi-monitor screen capture, fullscreen drag-capture overlay, and offline OCR chain (Apple Vision → Tesseract → Windows WinRT → built-in `ocrs`)
- `src/builtin_ocr.rs` — Pure-Rust offline OCR engine (`ocrs` + `rten`)
- `src/vision_ocr.rs` — Multimodal LLM image-to-translation pipeline
- `src/providers.rs` & `src/translator.rs` — Provider configurations, friendly error mapping, single-hop bilingual prompts, and translation orchestration
- `src/streaming.rs` — UTF-8-safe SSE line buffering and live token streaming for OpenAI, Gemini, Claude, DeepSeek, and Ollama
- `src/history.rs` & `src/glossary.rs` — Translation history (CSV/Markdown export) and custom glossary (CSV/JSON import & export)
- `src/secrets.rs` — OS Keychain / Credential Manager integration (`keyring`)
- `src/paths.rs` — Platform user data directory resolution and legacy config migration
- `src/theme.rs` — Graphite Dark & Graphite Light palettes (`unslop` tokens)
- `src/tts.rs` — Cross-platform native Text-to-Speech
- `src/clipboard_watch.rs` — Self-copy-aware clipboard watcher
- `src/translation_overlay.rs` — Floating HUD translation overlay
- `src/updater.rs` — GitHub Releases update checker and platform asset downloader
- `src/window_level.rs` — Runtime native Always-on-Top window level toggling
