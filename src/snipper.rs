use crate::builtin_ocr;
use crate::paths;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub const OCR_IMAGE_FILENAME: &str = "orc.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrLanguage {
    AutoDetect,
    ThaiEnglish,
    ChineseSimplifiedEnglish,
    ChineseTraditionalEnglish,
    JapaneseEnglish,
    KoreanEnglish,
    TurkishEnglish,
    ArabicEnglish,
    VietnameseEnglish,
}

impl OcrLanguage {
    pub const ALL: &'static [OcrLanguage] = &[
        OcrLanguage::AutoDetect,
        OcrLanguage::ThaiEnglish,
        OcrLanguage::ChineseSimplifiedEnglish,
        OcrLanguage::ChineseTraditionalEnglish,
        OcrLanguage::JapaneseEnglish,
        OcrLanguage::KoreanEnglish,
        OcrLanguage::TurkishEnglish,
        OcrLanguage::ArabicEnglish,
        OcrLanguage::VietnameseEnglish,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OcrLanguage::AutoDetect => "Auto-detect",
            OcrLanguage::ThaiEnglish => "Thai and English",
            OcrLanguage::ChineseSimplifiedEnglish => "Chinese and English",
            OcrLanguage::ChineseTraditionalEnglish => "Chinese (Trad.) and English",
            OcrLanguage::JapaneseEnglish => "Japanese and English",
            OcrLanguage::KoreanEnglish => "Korean and English",
            OcrLanguage::TurkishEnglish => "Turkish and English",
            OcrLanguage::ArabicEnglish => "Arabic and English",
            OcrLanguage::VietnameseEnglish => "Vietnamese and English",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "Auto-detect" => OcrLanguage::AutoDetect,
            "Thai and English" => OcrLanguage::ThaiEnglish,
            "Chinese (Trad.) and English" => OcrLanguage::ChineseTraditionalEnglish,
            "Japanese and English" => OcrLanguage::JapaneseEnglish,
            "Korean and English" => OcrLanguage::KoreanEnglish,
            "Turkish and English" => OcrLanguage::TurkishEnglish,
            "Arabic and English" => OcrLanguage::ArabicEnglish,
            "Vietnamese and English" => OcrLanguage::VietnameseEnglish,
            _ => OcrLanguage::ChineseSimplifiedEnglish,
        }
    }

    /// Language codes for `tesseract`.
    pub fn tesseract_lang(self) -> &'static str {
        match self {
            OcrLanguage::AutoDetect => "eng",
            OcrLanguage::ThaiEnglish => "tha+eng",
            OcrLanguage::ChineseSimplifiedEnglish => "chi_sim+eng",
            OcrLanguage::ChineseTraditionalEnglish => "chi_tra+eng",
            OcrLanguage::JapaneseEnglish => "jpn+eng",
            OcrLanguage::KoreanEnglish => "kor+eng",
            OcrLanguage::TurkishEnglish => "tur+eng",
            OcrLanguage::ArabicEnglish => "ara+eng",
            OcrLanguage::VietnameseEnglish => "vie+eng",
        }
    }

    /// BCP-47 language codes for macOS Vision `VNRecognizeTextRequest`.
    /// Empty slice signals `automaticallyDetectsLanguage = true`.
    pub fn apple_vision_langs(self) -> &'static [&'static str] {
        match self {
            OcrLanguage::AutoDetect => &[],
            OcrLanguage::ThaiEnglish => &["th-TH", "en-US"],
            OcrLanguage::ChineseSimplifiedEnglish => &["zh-Hans", "en-US"],
            OcrLanguage::ChineseTraditionalEnglish => &["zh-Hant", "en-US"],
            OcrLanguage::JapaneseEnglish => &["ja-JP", "en-US"],
            OcrLanguage::KoreanEnglish => &["ko-KR", "en-US"],
            OcrLanguage::TurkishEnglish => &["tr-TR", "en-US"],
            OcrLanguage::ArabicEnglish => &["ar-SA", "en-US"],
            OcrLanguage::VietnameseEnglish => &["vi-VN", "en-US"],
        }
    }

    /// Primary BCP-47 tag for Windows native WinRT `Windows.Media.Ocr.OcrEngine`.
    pub fn windows_ocr_lang(self) -> &'static str {
        match self {
            OcrLanguage::AutoDetect => "en-US",
            OcrLanguage::ThaiEnglish => "th-TH",
            OcrLanguage::ChineseSimplifiedEnglish => "zh-Hans-CN",
            OcrLanguage::ChineseTraditionalEnglish => "zh-Hant-TW",
            OcrLanguage::JapaneseEnglish => "ja-JP",
            OcrLanguage::KoreanEnglish => "ko-KR",
            OcrLanguage::TurkishEnglish => "tr-TR",
            OcrLanguage::ArabicEnglish => "ar-SA",
            OcrLanguage::VietnameseEnglish => "vi-VN",
        }
    }
}

/// Normalized region selected on the fullscreen drag-capture overlay (0.0..=1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl NormalizedRect {
    pub fn from_points(
        start_x: f32,
        start_y: f32,
        end_x: f32,
        end_y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<Self> {
        if viewport_w <= 0.0 || viewport_h <= 0.0 {
            return None;
        }
        let x1 = start_x.min(end_x).clamp(0.0, viewport_w);
        let y1 = start_y.min(end_y).clamp(0.0, viewport_h);
        let x2 = start_x.max(end_x).clamp(0.0, viewport_w);
        let y2 = start_y.max(end_y).clamp(0.0, viewport_h);
        let w = x2 - x1;
        let h = y2 - y1;
        if w < 4.0 || h < 4.0 {
            return None;
        }
        Some(Self {
            x: x1 / viewport_w,
            y: y1 / viewport_h,
            width: w / viewport_w,
            height: h / viewport_h,
        })
    }
}

#[derive(Debug, Error)]
pub enum SnipperError {
    #[error("Fullscreen capture failed: {0}")]
    CaptureFailed(String),
    #[error("Failed to crop selected region: {0}")]
    CropFailed(String),
    #[error("OCR failed: {0}")]
    OcrFailed(String),
}

/// Silently captures a specific display (1-based index on macOS/Windows, or primary when `None`).
pub fn capture_display_to_temp(display_index: Option<usize>) -> Result<PathBuf, SnipperError> {
    let temp_path = std::env::temp_dir().join(format!(
        "translate_app_fullscreen_{}.png",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temp_path);

    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("screencapture");
        cmd.arg("-x");
        if let Some(idx) = display_index {
            cmd.arg("-D").arg(idx.to_string());
        } else {
            cmd.arg("-m");
        }
        let status = cmd
            .arg(&temp_path)
            .status()
            .map_err(|e| SnipperError::CaptureFailed(e.to_string()))?;

        if !status.success() || !temp_path.exists() {
            return Err(SnipperError::CaptureFailed(
                "screencapture failed (check Screen Recording permission in System Settings)"
                    .to_string(),
            ));
        }
        Ok(temp_path)
    }

    #[cfg(target_os = "windows")]
    {
        let path_str = temp_path.to_string_lossy().replace('\'', "''");
        let idx = display_index.unwrap_or(1).saturating_sub(1);
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
             $screens = [System.Windows.Forms.Screen]::AllScreens; \
             $screen = if ({idx} -lt $screens.Length) {{ $screens[{idx}] }} else {{ [System.Windows.Forms.Screen]::PrimaryScreen }}; \
             $bounds = $screen.Bounds; \
             $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height; \
             $g = [System.Drawing.Graphics]::FromImage($bmp); \
             $g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size); \
             $bmp.Save('{path_str}', [System.Drawing.Imaging.ImageFormat]::Png); \
             $g.Dispose(); $bmp.Dispose()"
        );

        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &ps_script,
            ])
            .status()
            .map_err(|e| SnipperError::CaptureFailed(e.to_string()))?;

        if !status.success() || !temp_path.exists() {
            return Err(SnipperError::CaptureFailed(
                "Windows fullscreen capture failed".to_string(),
            ));
        }
        Ok(temp_path)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = display_index;
        let status = Command::new("gnome-screenshot")
            .arg("-f")
            .arg(&temp_path)
            .status()
            .map_err(|e| SnipperError::CaptureFailed(e.to_string()))?;

        if !status.success() || !temp_path.exists() {
            return Err(SnipperError::CaptureFailed(
                "Fullscreen capture tool failed".to_string(),
            ));
        }
        Ok(temp_path)
    }
}

/// Crops the user-dragged rectangle from `fullscreen_image_path` into the app cache directory
/// (`<data_dir>/cache/orc.png`) and cleans up the temp fullscreen image.
pub fn crop_selection_to_file(
    fullscreen_image_path: &Path,
    rect: NormalizedRect,
) -> Result<PathBuf, SnipperError> {
    let img =
        image::open(fullscreen_image_path).map_err(|e| SnipperError::CropFailed(e.to_string()))?;

    let img_w = img.width();
    let img_h = img.height();

    let crop_x = ((rect.x * img_w as f32).round() as u32).min(img_w.saturating_sub(1));
    let crop_y = ((rect.y * img_h as f32).round() as u32).min(img_h.saturating_sub(1));
    let crop_w = ((rect.width * img_w as f32).round() as u32)
        .max(1)
        .min(img_w.saturating_sub(crop_x));
    let crop_h = ((rect.height * img_h as f32).round() as u32)
        .max(1)
        .min(img_h.saturating_sub(crop_y));

    let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);
    let output_path = paths::cache_dir().join(OCR_IMAGE_FILENAME);
    cropped
        .save(&output_path)
        .map_err(|e| SnipperError::CropFailed(e.to_string()))?;

    let _ = std::fs::remove_file(fullscreen_image_path);
    Ok(output_path)
}

/// Crops the user-dragged rectangle, runs offline OCR, copies to clipboard, and returns text.
pub fn crop_and_ocr(
    fullscreen_image_path: &Path,
    rect: NormalizedRect,
    language: OcrLanguage,
) -> Result<String, SnipperError> {
    let output_path = crop_selection_to_file(fullscreen_image_path, rect)?;
    let text = run_ocr(&output_path, language)?;
    copy_to_clipboard(&text);
    Ok(text)
}

/// Parses `tesseract --list-langs` output into a list of installed language codes.
pub fn parse_tesseract_langs(output: &str) -> Vec<String> {
    output
        .lines()
        .skip(1)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Returns `true` only if every language in `spec` (e.g. `"tha+eng"`) is present in `installed`.
pub fn tesseract_supports_all_langs(spec: &str, installed: &[String]) -> bool {
    spec.split('+')
        .filter(|s| !s.is_empty())
        .all(|needed| installed.iter().any(|lang| lang == needed))
}

fn run_tesseract_if_supported(image_path: &Path, lang_spec: &str) -> Option<String> {
    let list_out = Command::new("tesseract")
        .arg("--list-langs")
        .output()
        .ok()?;
    if !list_out.status.success() {
        return None;
    }
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&list_out.stdout),
        String::from_utf8_lossy(&list_out.stderr)
    );
    let installed = parse_tesseract_langs(&combined);
    if !tesseract_supports_all_langs(lang_spec, &installed) {
        return None;
    }

    let output = Command::new("tesseract")
        .arg(image_path)
        .arg("stdout")
        .args(["-l", lang_spec])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("Failed loading language") {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Offline OCR chain:
/// - macOS: Apple Vision (`VNRecognizeTextRequest`, accurate Thai/CJK/Arabic/Latin natively) → `tesseract` (if all requested languages installed) → built-in `ocrs`
/// - Windows: `tesseract` (if all requested languages installed) → native WinRT `Windows.Media.Ocr.OcrEngine` → built-in `ocrs`
/// - Linux: `tesseract` (if all requested languages installed) → built-in `ocrs`
pub fn run_ocr(image_path: &Path, language: OcrLanguage) -> Result<String, SnipperError> {
    #[cfg(target_os = "macos")]
    {
        let langs = language.apple_vision_langs();
        let lang_setup = if langs.is_empty() {
            "request.automaticallyDetectsLanguage = true".to_string()
        } else {
            let literal = langs
                .iter()
                .map(|l| format!("\"{l}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!("request.recognitionLanguages = [{literal}]")
        };

        let swift_script = format!(
            r#"
import Foundation
import Vision
import AppKit

let url = URL(fileURLWithPath: CommandLine.arguments[1])
guard let image = NSImage(contentsOf: url),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {{
    exit(1)
}}
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
{lang_setup}
request.usesLanguageCorrection = true
let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
try handler.perform([request])
let lines = (request.results ?? []).compactMap {{ $0.topCandidates(1).first?.string }}
print(lines.joined(separator: " "))
"#
        );

        if let Ok(output) = Command::new("swift")
            .arg("-e")
            .arg(&swift_script)
            .arg(image_path)
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    if let Some(text) = run_tesseract_if_supported(image_path, language.tesseract_lang()) {
        return Ok(text);
    }

    #[cfg(target_os = "windows")]
    {
        let abs_path =
            std::fs::canonicalize(image_path).unwrap_or_else(|_| image_path.to_path_buf());
        let path_str = abs_path.to_string_lossy().replace('\'', "''");
        let lang_tag = language.windows_ocr_lang();
        let ps_script = format!(
            r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null = [Windows.Storage.StorageFile, Windows.Storage, ContentType=WindowsRuntime]
$null = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType=WindowsRuntime]
$null = [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType=WindowsRuntime]
$null = [Windows.Globalization.Language, Windows.Globalization, ContentType=WindowsRuntime]
function Await($op, $type) {{
    $method = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' }} | Select-Object -First 1
    $task = $method.MakeGenericMethod($type).Invoke($null, @($op))
    $task.Wait()
    return $task.Result
}}
$file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync('{path_str}')) ([Windows.Storage.StorageFile])
$stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
$decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
$bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
$lang = New-Object Windows.Globalization.Language('{lang_tag}')
$engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($lang)
if ($null -eq $engine) {{ $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages() }}
$res = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
Write-Output $res.Text
"#
        );

        if let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &ps_script,
            ])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    // Pure-Rust offline fallback (`ocrs` + `rten` ONNX models in `<data_dir>/models`)
    if let Ok(text) = builtin_ocr::recognize_image(image_path) {
        if !text.is_empty() {
            return Ok(text);
        }
    }

    Err(SnipperError::OcrFailed(
        "No offline OCR engine succeeded (for Tesseract, install language packs e.g. `brew install tesseract-lang`, or enable Vision AI OCR in Settings)"
            .to_string(),
    ))
}

pub fn copy_to_clipboard(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

pub fn read_from_clipboard() -> String {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.get_text())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_ocr_language_codes() {
        assert_eq!(OcrLanguage::ALL.len(), 9);
        assert!(OcrLanguage::AutoDetect.apple_vision_langs().is_empty());
        assert_eq!(OcrLanguage::ThaiEnglish.tesseract_lang(), "tha+eng");
        assert_eq!(OcrLanguage::JapaneseEnglish.tesseract_lang(), "jpn+eng");
        assert_eq!(
            OcrLanguage::KoreanEnglish.apple_vision_langs(),
            &["ko-KR", "en-US"]
        );
        assert_eq!(OcrLanguage::ArabicEnglish.windows_ocr_lang(), "ar-SA");
        assert_eq!(
            OcrLanguage::from_label("Japanese and English"),
            OcrLanguage::JapaneseEnglish
        );
        assert_eq!(
            OcrLanguage::from_label("Auto-detect"),
            OcrLanguage::AutoDetect
        );
    }

    #[test]
    fn normalizes_dragged_selection_rectangle() {
        let rect = NormalizedRect::from_points(200.0, 100.0, 50.0, 300.0, 1000.0, 500.0).unwrap();
        assert!((rect.x - 0.05).abs() < 1e-6);
        assert!((rect.y - 0.20).abs() < 1e-6);
        assert!((rect.width - 0.15).abs() < 1e-6);
        assert!((rect.height - 0.40).abs() < 1e-6);
    }

    #[test]
    fn rejects_tiny_accidental_click() {
        assert!(NormalizedRect::from_points(10.0, 10.0, 12.0, 11.0, 1000.0, 500.0).is_none());
    }

    #[test]
    fn tesseract_lang_check_rejects_missing_thai_pack() {
        let output = "List of available languages in \"/opt/homebrew/share/tessdata/\" (3):\neng\nosd\nsnum\n";
        let installed = parse_tesseract_langs(output);
        assert_eq!(installed, vec!["eng", "osd", "snum"]);
        assert!(tesseract_supports_all_langs("eng", &installed));
        assert!(!tesseract_supports_all_langs("tha+eng", &installed));
        assert!(!tesseract_supports_all_langs("chi_sim+eng", &installed));

        let with_thai = vec!["eng".into(), "tha".into()];
        assert!(tesseract_supports_all_langs("tha+eng", &with_thai));
    }
}
