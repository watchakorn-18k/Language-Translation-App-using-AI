<p align="center"><img src="src/icon.ico" width="50" alt="App Icon"></p>

# Language Translation App with AI (Rust + GPUI Kit)

A fast, native desktop translation utility written in **Rust** with **[gpui-kit](https://github.com/longbridge/gpui-kit)**, featuring OS-wide global drag-capture OCR, Vision AI OCR, live streaming translations, custom glossary, history, OS Keychain credential storage, and bilingual output (`EN` + customizable target language).

<p align="center">
  <img src="screenshots/example.png" height="460" alt="Main Screen">
  <img src="screenshots/settings.png" height="460" alt="Settings Dialog">
</p>

## Features

- **Native GPU-Accelerated Desktop UI:** Built with Rust and `gpui-kit` (`0.10.0`), featuring a quiet Graphite dark interface, bilingual two-tier translation sheet (`EN` → target language), one-click copy, and searchable target language picker (100+ languages).
- **OS-Wide Global Hotkey & Fullscreen Drag-Capture OCR (`⌘⇧Z` / `Ctrl+Shift+Z`):**
  - Trigger fullscreen region capture from any application (games, PDFs, browsers).
  - Multi-monitor aware overlay (`SnipperOverlay`) with crosshair cursor, un-dimmed selection cutout, and pure-Rust cropping (`image`).
  - **Auto-translate after OCR** option translates immediately on mouse release.
  - **Zero-install Offline OCR chain:** `tesseract` CLI → macOS Apple Vision (`VNRecognizeTextRequest`) → Windows native WinRT `Windows.Media.Ocr.OcrEngine`.
  - **Vision AI OCR mode:** Optionally send the cropped image directly to multimodal models (`gemini-4-neon`, `gpt-6.2-sol`, `claude-sonnet-5-5`, `gemma4`) to read and translate in a single request.
- **Modern Translation Engines (2026 Lineup, Configurable In-App):**
  - **Google Translate** — Free, instant, no API key required
  - **OpenAI** — Default `gpt-6.2-sol` (supports `gpt-6.2-astra`, `gpt-6.1-terra`, `gpt-6-luna`, etc.)
  - **Google Gemini** — Default `gemini-4-neon` (supports `gemini-4-hydrogen`, `gemini-4-argon`, `gemini-4-carbon`, etc.)
  - **Claude (Anthropic)** — Default `claude-sonnet-5-5` (supports `claude-opus-5-5`, `claude-haiku-5-5`, `claude-fable-5-2`)
  - **DeepSeek** — Default `deepseek-chat` (supports `deepseek-reasoner`)
  - **Ollama (Local AI)** — Free local models (default `gemma4`, also `qwen3.5:9b`, `qwen3.5:4b` at `http://localhost:11434/v1`)
  - **Custom (OpenAI-Compatible API)** — Works with OpenRouter, Groq, LM Studio, Together, vLLM, etc.
- **Single-Hop Direct AI Translation & Live Streaming:** Translate into both English and your target language in a single API call with live SSE token streaming.
- **Translation History & Custom Glossary:**
  - Searchable history (`translation_history.json`, up to 200 entries) accessible from the header clock icon.
  - Custom glossary (`glossary.json`) automatically injected into AI system prompts for consistent proper nouns, character names, and terminology.
- **OS Keychain Security & In-App Update Check:**
  - API keys stored securely in **macOS Keychain** / **Windows Credential Manager** via `keyring` (with transparent local JSON fallback).
  - Automatic background check against GitHub Releases for newer versions.

## Quick Start

### Prerequisites

- **Rust** (stable toolchain, `cargo`)
- *(Optional)* **Tesseract OCR** (`brew install tesseract tesseract-lang` on macOS). On macOS and Windows 10/11, native OS OCR works out of the box without extra installs.

### Run the App

```bash
cargo run
```

### Build Release Binary

```bash
cargo build --release
```

### Run Unit Tests

```bash
cargo test --lib
```

## Configuration

Open **Settings** by clicking the engine badge or gear icon in the bottom dock:

1. Choose your **Translator** provider.
2. Enter the **Model**, **API key** (masked, stored in OS Keychain when enabled), and **Base URL** (for Ollama or Custom OpenAI-compatible endpoints).
3. Toggle **Auto-translate after OCR**, **Direct AI translation (single-hop)**, **Vision AI OCR**, **Stream responses**, and **Store API keys in OS Keychain**.
4. Click **Save**.

> All local config, history, glossary, and API key JSON files are excluded via `.gitignore`.

## Releases & CI

Pushing a version tag (`v*`) triggers `.github/workflows/release.yml` (with `Swatinem/rust-cache@v2` caching):
- **macOS Apple Silicon (`aarch64-apple-darwin`):** `.app` bundle + `.dmg` installer + `.tar.gz` + `.sha256`
- **Windows x64 (`x86_64-pc-windows-msvc`):** `.exe` with embedded `src/icon.ico` + `.zip` + `.sha256`

## Credit

- Original Python Snipper concept inspired by [textshot](https://github.com/ianzhao05/textshot)
- UI redesign guided by `unslop` by Damir Sokolovsky ([@heydamir](https://x.com/heydamir))

## Changelog

- **6.3 (Phases 1–3 Complete):**
  - Added OS-wide global hotkey (`global-hotkey`) for `⌘⇧Z` / `Ctrl+Shift+Z`
  - Added Auto-translate after OCR, Single-Hop Direct AI Translation, and Always-on-Top pin preference
  - Added Vision AI OCR mode (multimodal image-to-translation), live SSE streaming output, multi-monitor drag-capture support, Translation History, and Custom Glossary
  - Added OS Keychain credential storage (`keyring`), Windows native WinRT offline OCR, embedded Windows `.exe` icon (`build.rs`), in-app GitHub Release update checker, and macOS `.app` + `.dmg` CI packaging
  - Expanded unit test suite to 25 tests covering all core modules
- **6.0 (Rust + GPUI Kit Rewrite):**
  - Rewrote the entire application in Rust using `gpui-kit 0.10.0`
  - Redesigned UI with Graphite dark theme, bilingual two-tier translation sheet, ISO language tags, and shortcut keycaps
  - Rebuilt fullscreen drag-capture OCR overlay natively in GPUI (`SnipperOverlay`)
  - Replaced retired providers with modern 2026 AI models (`gpt-6.2-sol`, `gemini-4-neon`, `claude-sonnet-5-5`, `deepseek-chat`, `gemma4`)
- **1.0 – 5.2:** Legacy Python (Flet / PyQt5) releases
