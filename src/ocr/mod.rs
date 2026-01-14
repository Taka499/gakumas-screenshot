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

/// High-level function: screenshot → scores
pub fn ocr_screenshot(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, threshold: u8) -> Result<[[u32; 3]; 3]> {
    let preprocessed = threshold_bright_pixels(img, threshold);
    let lines = recognize_image(&preprocessed)?;
    extract_scores(&lines)
}
