pub mod setup;
pub mod preprocess;
pub mod engine;
pub mod extract;

pub use setup::ensure_tesseract;
pub use preprocess::threshold_bright_pixels;
pub use engine::{recognize_image, OcrLine, OcrWord};
pub use extract::extract_scores;

use anyhow::Result;
use image::{ImageBuffer, Rgba};

use crate::automation::config::RelativeRect;
use preprocess::crop_region;
use engine::recognize_image_line;
use extract::extract_single_stage;

/// High-level function: screenshot → scores using per-stage cropping.
///
/// For each of the 3 stages, crops the score region, preprocesses,
/// runs single-line OCR, and extracts scores.
pub fn ocr_screenshot(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    threshold: u8,
    score_regions: &[RelativeRect; 3],
) -> Result<[[u32; 3]; 3]> {
    let mut scores = [[0u32; 3]; 3];

    for (stage_idx, region) in score_regions.iter().enumerate() {
        crate::log(&format!(
            "OCR stage {}: cropping region y={:.3} h={:.3}",
            stage_idx + 1, region.y, region.height
        ));

        let cropped = crop_region(img, region);
        let preprocessed = threshold_bright_pixels(&cropped, threshold);
        let lines = recognize_image_line(&preprocessed)?;
        scores[stage_idx] = extract_single_stage(&lines)?;
    }

    Ok(scores)
}
