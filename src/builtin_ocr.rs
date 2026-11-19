use crate::paths;
use std::path::{Path, PathBuf};

pub const DETECTION_MODEL_FILE: &str = "text-detection.onnx";
pub const RECOGNITION_MODEL_FILE: &str = "text-recognition.onnx";
pub const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
pub const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";

/// Returns `(detection_model_path, recognition_model_path)` inside the user's models directory.
pub fn model_paths_in(dir: &Path) -> (PathBuf, PathBuf) {
    (
        dir.join(DETECTION_MODEL_FILE),
        dir.join(RECOGNITION_MODEL_FILE),
    )
}

pub fn models_present_in(dir: &Path) -> bool {
    let (det, rec) = model_paths_in(dir);
    det.is_file() && rec.is_file()
}

/// Runs pure-Rust offline OCR (`ocrs` + `rten`) if the `builtin-ocr` feature is enabled
/// and the ONNX models are present in `paths::models_dir()`.
#[cfg(feature = "builtin-ocr")]
pub fn recognize_image(image_path: &Path) -> Result<String, String> {
    use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
    use rten::Model;

    let dir = paths::models_dir();
    let (det_path, rec_path) = model_paths_in(&dir);
    if !det_path.is_file() || !rec_path.is_file() {
        return Err(format!(
            "Built-in OCR models not found in {}",
            dir.display()
        ));
    }

    let detection_model =
        Model::load_file(&det_path).map_err(|e| format!("Failed to load detection model: {e}"))?;
    let recognition_model = Model::load_file(&rec_path)
        .map_err(|e| format!("Failed to load recognition model: {e}"))?;

    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|e| format!("Failed to initialize ocrs engine: {e}"))?;

    let img = image::open(image_path)
        .map_err(|e| format!("Failed to open image: {e}"))?
        .into_rgb8();
    let img_source = ImageSource::from_bytes(img.as_raw(), img.dimensions())
        .map_err(|e| format!("Invalid image source: {e}"))?;
    let ocr_input = engine
        .prepare_input(img_source)
        .map_err(|e| format!("Failed to prepare OCR input: {e}"))?;

    engine
        .get_text(&ocr_input)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("OCR inference failed: {e}"))
}

#[cfg(not(feature = "builtin-ocr"))]
pub fn recognize_image(_image_path: &Path) -> Result<String, String> {
    Err("Built-in OCR feature is disabled".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolves_and_checks_model_presence() {
        let dir = tempdir().unwrap();
        assert!(!models_present_in(dir.path()));
        let (det, rec) = model_paths_in(dir.path());
        fs::write(&det, b"dummy").unwrap();
        assert!(!models_present_in(dir.path()));
        fs::write(&rec, b"dummy").unwrap();
        assert!(models_present_in(dir.path()));
    }
}
