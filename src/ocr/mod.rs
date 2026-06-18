pub mod setup;
pub mod preprocess;
pub mod engine;
pub mod extract;
pub mod reconcile;

pub use setup::ensure_tesseract;
pub use preprocess::threshold_bright_pixels;
pub use engine::{recognize_image, OcrLine, OcrWord};
pub use extract::extract_scores;
pub use reconcile::Recovery;

use anyhow::Result;
use image::{ImageBuffer, Rgba};

use crate::automation::config::RelativeRect;
use preprocess::{blue_mask, crop_region};
use engine::{recognize_image_line, recognize_single_number};
use extract::extract_single_stage;

/// Per-stage OCR readout: the nine per-character scores plus the isolated
/// stage total and bonus badge that drive checksum reconstruction.
///
/// `scores` holds the (post-reconciliation, once M4 wires it) per-character
/// values. `totals`/`bonuses` are `None` when that isolated number failed to
/// OCR or looked like over-detected garbage. `flags` records each stage's
/// reconstruction confidence (default `Recovery::Ok` until M3/M4 fill it).
#[derive(Clone, Copy, Debug)]
pub struct StageReadout {
    pub scores: [[u32; 3]; 3],
    pub totals: [Option<u32>; 3],
    pub bonuses: [Option<u32>; 3],
    pub flags: [Recovery; 3],
}

/// High-level function: screenshot → per-stage readout using per-stage cropping.
///
/// For each of the 3 stages, crops and OCRs the score row, the isolated stage
/// total (white text, luminance threshold), and the bonus badge (light-blue
/// text, blue-selective mask). The preprocessing thresholds are read from the
/// global config (`ocr_threshold`, `total_threshold`, `bonus_blue_min`,
/// `bonus_br_margin`). The total/bonus feed the checksum reconstruction (M3/M4);
/// a failed total/bonus reads as `None` and simply disables the checksum tier.
pub fn ocr_screenshot(
    img: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    score_regions: &[RelativeRect; 3],
    total_regions: &[RelativeRect; 3],
    bonus_regions: &[RelativeRect; 3],
) -> Result<StageReadout> {
    let config = crate::automation::config::get_config();
    let threshold = config.ocr_threshold;
    let total_threshold = config.total_threshold;
    let bonus_blue_min = config.bonus_blue_min;
    let bonus_br_margin = config.bonus_br_margin;

    let mut readout = StageReadout {
        scores: [[0u32; 3]; 3],
        totals: [None; 3],
        bonuses: [None; 3],
        flags: [Recovery::Ok; 3],
    };

    for stage_idx in 0..3 {
        // Score row.
        let score_crop = crop_region(img, &score_regions[stage_idx]);
        let score_bin = threshold_bright_pixels(&score_crop, threshold);
        let lines = recognize_image_line(&score_bin)?;
        readout.scores[stage_idx] = extract_single_stage(&lines)?;

        // Stage total: white text, same luminance threshold style as score rows.
        let total_crop = crop_region(img, &total_regions[stage_idx]);
        let total_bin = threshold_bright_pixels(&total_crop, total_threshold);
        readout.totals[stage_idx] = recognize_single_number(&total_bin, "0123456789,", false)?;

        // Bonus badge: light-blue text, blue-selective mask, "+"-anchored parse.
        let bonus_crop = crop_region(img, &bonus_regions[stage_idx]);
        let bonus_bin = blue_mask(&bonus_crop, bonus_blue_min, bonus_br_margin);
        readout.bonuses[stage_idx] = recognize_single_number(&bonus_bin, "0123456789+", true)?;

        crate::log(&format!(
            "OCR stage {}: scores={:?} total={:?} bonus={:?}",
            stage_idx + 1,
            readout.scores[stage_idx],
            readout.totals[stage_idx],
            readout.bonuses[stage_idx]
        ));
    }

    Ok(readout)
}
