use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub const OCR_IMAGE_PATH: &str = "orc.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrLanguage {
    ChineseEnglish,
    ThaiEnglish,
    TurkishEnglish,
}

impl OcrLanguage {
    pub const ALL: &'static [OcrLanguage] = &[
        OcrLanguage::ChineseEnglish,
        OcrLanguage::ThaiEnglish,
        OcrLanguage::TurkishEnglish,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OcrLanguage::ChineseEnglish => "Chinese and English",
            OcrLanguage::ThaiEnglish => "Thai and English",
            OcrLanguage::TurkishEnglish => "Turkish and English",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "Thai and English" => OcrLanguage::ThaiEnglish,
            "Turkish and English" => OcrLanguage::TurkishEnglish,
            _ => OcrLanguage::ChineseEnglish,
        }
    }

    /// Language codes matching `src/Snippers.py` (`easyocr`).
    pub fn easyocr_codes(self) -> &'static [&'static str] {
        match self {
            OcrLanguage::ChineseEnglish => &["ch_sim", "en"],
            OcrLanguage::ThaiEnglish => &["th", "en"],
            OcrLanguage::TurkishEnglish => &["tr", "en"],
        }
    }

    /// Language codes for `tesseract`.
    pub fn tesseract_lang(self) -> &'static str {
        match self {
            OcrLanguage::ChineseEnglish => "chi_sim+eng",
            OcrLanguage::ThaiEnglish => "tha+eng",
            OcrLanguage::TurkishEnglish => "tur+eng",
        }
    }

    /// BCP-47 language codes for macOS Vision `VNRecognizeTextRequest`.
    pub fn apple_vision_langs(self) -> &'static [&'static str] {
        match self {
            OcrLanguage::ChineseEnglish => &["zh-Hans", "en-US"],
            OcrLanguage::ThaiEnglish => &["th-TH", "en-US"],
            OcrLanguage::TurkishEnglish => &["tr-TR", "en-US"],
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
    #[error("OCR failed (install `tesseract` or run on macOS with Vision): {0}")]
    OcrFailed(String),
}

/// Silently captures the full primary screen to a temporary PNG file for the drag-capture overlay.
pub fn capture_fullscreen_to_temp() -> Result<PathBuf, SnipperError> {
    let temp_path = std::env::temp_dir().join(format!(
        "translate_app_fullscreen_{}.png",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temp_path);

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("screencapture")
            .args(["-x", "-m"])
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
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; \
             $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; \
             $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height; \
             $g = [System.Drawing.Graphics]::FromImage($bmp); \
             $g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size); \
             $bmp.Save('{path_str}', [System.Drawing.Imaging.ImageFormat]::Png); \
             $g.Dispose(); $bmp.Dispose()"
        );

        let status = Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_script])
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

/// Crops the user-dragged rectangle from the captured fullscreen PNG into `orc.png`,
/// runs OCR on it, copies the extracted text to the clipboard, and returns the text.
/// (Direct Rust port of `Snipper.ocrOfDrawnRectangle()` in `src/Snippers.py`.)
pub fn crop_and_ocr(
    fullscreen_image_path: &Path,
    rect: NormalizedRect,
    language: OcrLanguage,
) -> Result<String, SnipperError> {
    let img = image::open(fullscreen_image_path)
        .map_err(|e| SnipperError::CropFailed(e.to_string()))?;

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
    let output_path = Path::new(OCR_IMAGE_PATH);
    cropped
        .save(output_path)
        .map_err(|e| SnipperError::CropFailed(e.to_string()))?;

    let _ = std::fs::remove_file(fullscreen_image_path);

    let text = run_ocr(output_path, language)?;
    copy_to_clipboard(&text);
    Ok(text)
}

pub fn run_ocr(image_path: &Path, language: OcrLanguage) -> Result<String, SnipperError> {
    // 1. Try tesseract first if available on PATH.
    if let Ok(output) = Command::new("tesseract")
        .arg(image_path)
        .arg("stdout")
        .args(["-l", language.tesseract_lang()])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(text);
        }
    }

    // 2. Fallback on macOS: Apple Vision framework via `swift`.
    #[cfg(target_os = "macos")]
    {
        let langs_literal = language
            .apple_vision_langs()
            .iter()
            .map(|l| format!("\"{l}\""))
            .collect::<Vec<_>>()
            .join(", ");

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
request.recognitionLanguages = [{langs_literal}]
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
                return Ok(text);
            }
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(SnipperError::OcrFailed(err));
        }
    }

    Err(SnipperError::OcrFailed(
        "Neither `tesseract` nor macOS Vision OCR succeeded".to_string(),
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
    fn maps_ocr_language_codes() {
        assert_eq!(OcrLanguage::ChineseEnglish.easyocr_codes(), &["ch_sim", "en"]);
        assert_eq!(OcrLanguage::ThaiEnglish.tesseract_lang(), "tha+eng");
        assert_eq!(
            OcrLanguage::from_label("Turkish and English"),
            OcrLanguage::TurkishEnglish
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
}
