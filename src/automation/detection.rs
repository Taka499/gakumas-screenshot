//! Loading state detection via histogram comparison and brightness analysis.
//!
//! This module provides functions to detect when the game finishes loading
//! by monitoring screen regions using:
//! - Histogram comparison: Detect when Skip button appears (matches reference image)
//! - Brightness analysis: Detect when Skip button becomes enabled (not dimmed)

use anyhow::{anyhow, Result};
use image::{ImageBuffer, Rgba};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;

use crate::automation::config::AutomationConfig;
use crate::automation::state::ABORT_REQUESTED;
use crate::capture::region::capture_region;

/// Calculates the average brightness (luminance) of an image.
///
/// Uses the ITU-R BT.601 luma formula: Y = 0.299*R + 0.587*G + 0.114*B
/// Returns a value from 0.0 (black) to 255.0 (white).
pub fn calculate_brightness(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> f32 {
    if img.width() == 0 || img.height() == 0 {
        return 0.0;
    }

    let mut total: f64 = 0.0;
    let pixel_count = (img.width() * img.height()) as f64;

    for pixel in img.pixels() {
        let r = pixel[0] as f64;
        let g = pixel[1] as f64;
        let b = pixel[2] as f64;
        let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
        total += luminance;
    }

    (total / pixel_count) as f32
}

/// Calculates a grayscale histogram for an image.
///
/// Returns an array of 256 bins representing the distribution of pixel intensities.
/// Each bin is normalized to [0.0, 1.0] range.
fn calculate_histogram(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> [f32; 256] {
    let mut histogram = [0u32; 256];
    let pixel_count = (img.width() * img.height()) as f32;

    if pixel_count == 0.0 {
        return [0.0; 256];
    }

    for pixel in img.pixels() {
        // Convert to grayscale using luminance formula
        let gray = (0.299 * pixel[0] as f32
            + 0.587 * pixel[1] as f32
            + 0.114 * pixel[2] as f32) as u8;
        histogram[gray as usize] += 1;
    }

    // Normalize to [0.0, 1.0]
    let mut normalized = [0.0f32; 256];
    for i in 0..256 {
        normalized[i] = histogram[i] as f32 / pixel_count;
    }
    normalized
}

/// Calculates histogram similarity using Bhattacharyya coefficient.
///
/// Returns a value from 0.0 (completely different) to 1.0 (identical).
/// This metric is robust to lighting changes and works well for template matching.
fn histogram_similarity(hist1: &[f32; 256], hist2: &[f32; 256]) -> f32 {
    let mut bc = 0.0f32;
    for i in 0..256 {
        bc += (hist1[i] * hist2[i]).sqrt();
    }
    bc
}

/// Loads a reference image from disk and calculates its histogram.
pub fn load_reference_histogram(path: &Path) -> Result<[f32; 256]> {
    let img = image::open(path)
        .map_err(|e| anyhow!("Failed to load reference image {}: {}", path.display(), e))?
        .to_rgba8();
    Ok(calculate_histogram(&img))
}

/// Saves the current start button region as a reference image.
pub fn save_start_button_reference(hwnd: HWND, config: &AutomationConfig, path: &Path) -> Result<()> {
    let region_img = capture_region(hwnd, &config.start_button_region)?;
    region_img.save(path)
        .map_err(|e| anyhow!("Failed to save reference image: {}", e))?;
    crate::log(&format!("Saved Start button reference to {}", path.display()));
    Ok(())
}

/// Saves the current skip button region as a reference image.
pub fn save_skip_button_reference(hwnd: HWND, config: &AutomationConfig, path: &Path) -> Result<()> {
    let region_img = capture_region(hwnd, &config.skip_button_region)?;
    region_img.save(path)
        .map_err(|e| anyhow!("Failed to save reference image: {}", e))?;
    crate::log(&format!("Saved Skip button reference to {}", path.display()));
    Ok(())
}

/// Saves the current end button region as a reference image.
pub fn save_end_button_reference(hwnd: HWND, config: &AutomationConfig, path: &Path) -> Result<()> {
    let region_img = capture_region(hwnd, &config.end_button_region)?;
    region_img.save(path)
        .map_err(|e| anyhow!("Failed to save reference image: {}", e))?;
    crate::log(&format!("Saved End button reference to {}", path.display()));
    Ok(())
}

/// Waits for loading to complete using two-phase detection.
///
/// Phase 1: Wait for Skip button to appear (histogram matches reference image)
/// Phase 2: Wait for Skip button to become enabled (brightness exceeds threshold)
///
/// If no reference image exists, falls back to brightness-only detection.
pub fn wait_for_loading(hwnd: HWND, config: &AutomationConfig) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_millis(config.loading_timeout_ms);

    // Try to load reference histogram
    let ref_path = crate::paths::get_exe_dir().join(&config.skip_button_reference);

    let reference_histogram = if ref_path.exists() {
        match load_reference_histogram(&ref_path) {
            Ok(hist) => {
                crate::log(&format!(
                    "Loaded Skip button reference from {}",
                    ref_path.display()
                ));
                Some(hist)
            }
            Err(e) => {
                crate::log(&format!(
                    "Warning: Failed to load reference image: {}. Using brightness-only detection.",
                    e
                ));
                None
            }
        }
    } else {
        crate::log(&format!(
            "Warning: Reference image {} not found. Using brightness-only detection.",
            ref_path.display()
        ));
        crate::log("Hint: Use 'Capture Skip Reference' from tray menu to create it.");
        None
    };

    // Phase 1: Wait for Skip button to appear (if reference exists)
    if let Some(ref ref_hist) = reference_histogram {
        crate::log("Phase 1: Waiting for Skip button to appear...");
        loop {
            if ABORT_REQUESTED.load(Ordering::SeqCst) {
                return Err(anyhow!("Abort requested"));
            }

            if start.elapsed() > timeout {
                return Err(anyhow!(
                    "Timeout waiting for Skip button (phase 1) after {}ms",
                    config.loading_timeout_ms
                ));
            }

            let region_img = capture_region(hwnd, &config.skip_button_region)?;
            let current_hist = calculate_histogram(&region_img);
            let similarity = histogram_similarity(ref_hist, &current_hist);

            crate::log(&format!(
                "Phase 1: similarity = {:.3} (threshold = {:.3})",
                similarity, config.histogram_threshold
            ));

            if similarity >= config.histogram_threshold {
                crate::log("Skip button detected (histogram match)");
                break;
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // Phase 2: Wait for Skip button to become enabled (brightness)
    crate::log("Phase 2: Waiting for Skip button to become enabled...");
    loop {
        if ABORT_REQUESTED.load(Ordering::SeqCst) {
            return Err(anyhow!("Abort requested"));
        }

        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for Skip enabled (phase 2) after {}ms",
                config.loading_timeout_ms
            ));
        }

        let region_img = capture_region(hwnd, &config.skip_button_region)?;
        let brightness = calculate_brightness(&region_img);

        crate::log(&format!(
            "Phase 2: brightness = {:.2} (threshold = {:.2})",
            brightness, config.brightness_threshold
        ));

        if brightness > config.brightness_threshold {
            crate::log("Skip button enabled (brightness exceeded threshold)");
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Captures a region and returns its brightness value.
///
/// This is a convenience function for calibration - it captures the specified
/// region and calculates its brightness without any threshold checking.
pub fn measure_region_brightness(hwnd: HWND, config: &AutomationConfig) -> Result<f32> {
    let region_img = capture_region(hwnd, &config.skip_button_region)?;
    Ok(calculate_brightness(&region_img))
}

/// Waits for the result page to appear by detecting the "終了" (End) button.
///
/// Uses histogram comparison against a reference image of the End button region.
/// Returns Ok(()) when the End button is detected, or Err on timeout or abort.
///
/// If no reference image exists, falls back to a fixed delay.
pub fn wait_for_result(hwnd: HWND, config: &AutomationConfig) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_millis(config.result_timeout_ms);

    // Try to load reference histogram
    let ref_path = crate::paths::get_exe_dir().join(&config.end_button_reference);

    let reference_histogram = if ref_path.exists() {
        match load_reference_histogram(&ref_path) {
            Ok(hist) => {
                crate::log(&format!(
                    "Loaded End button reference from {}",
                    ref_path.display()
                ));
                Some(hist)
            }
            Err(e) => {
                crate::log(&format!(
                    "Warning: Failed to load End button reference: {}. Using fixed delay.",
                    e
                ));
                None
            }
        }
    } else {
        crate::log(&format!(
            "Warning: End button reference {} not found. Using fixed delay.",
            ref_path.display()
        ));
        crate::log("Hint: Use 'Capture End Reference' from tray menu to create it.");
        None
    };

    // If no reference, use fixed delay fallback
    if reference_histogram.is_none() {
        crate::log(&format!(
            "Waiting {} ms for result page (no reference image)...",
            config.capture_delay_ms
        ));
        std::thread::sleep(Duration::from_millis(config.capture_delay_ms));
        return Ok(());
    }

    let ref_hist = reference_histogram.unwrap();

    // Wait for End button to appear (histogram comparison)
    crate::log("Waiting for End button to appear (result page)...");
    loop {
        if ABORT_REQUESTED.load(Ordering::SeqCst) {
            return Err(anyhow!("Abort requested"));
        }

        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for result page after {}ms",
                config.result_timeout_ms
            ));
        }

        let region_img = capture_region(hwnd, &config.end_button_region)?;
        let current_hist = calculate_histogram(&region_img);
        let similarity = histogram_similarity(&ref_hist, &current_hist);

        crate::log(&format!(
            "Result page detection: similarity = {:.3} (threshold = {:.3})",
            similarity, config.histogram_threshold
        ));

        if similarity >= config.histogram_threshold {
            crate::log("End button detected (result page loaded)");
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Waits for the rehearsal start page to appear by detecting the "開始する" (Start) button.
///
/// Uses histogram comparison against a reference image of the Start button region.
/// Returns Ok(()) when the Start button is detected, or Err on timeout or abort.
///
/// If no reference image exists, returns immediately (assumes page is ready).
pub fn wait_for_start_page(hwnd: HWND, config: &AutomationConfig) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_millis(config.loading_timeout_ms);

    // Try to load reference histogram
    let ref_path = crate::paths::get_exe_dir().join(&config.start_button_reference);

    let reference_histogram = if ref_path.exists() {
        match load_reference_histogram(&ref_path) {
            Ok(hist) => {
                crate::log(&format!(
                    "Loaded Start button reference from {}",
                    ref_path.display()
                ));
                Some(hist)
            }
            Err(e) => {
                crate::log(&format!(
                    "Warning: Failed to load Start button reference: {}. Skipping page detection.",
                    e
                ));
                None
            }
        }
    } else {
        crate::log(&format!(
            "Warning: Start button reference {} not found. Skipping page detection.",
            ref_path.display()
        ));
        crate::log("Hint: Use 'Capture Start Reference' from tray menu to create it.");
        None
    };

    // If no reference, skip detection (assume we're on the right page)
    if reference_histogram.is_none() {
        return Ok(());
    }

    let ref_hist = reference_histogram.unwrap();

    // Wait for Start button to appear (histogram comparison)
    crate::log("Waiting for Start button to appear (rehearsal page)...");
    loop {
        if ABORT_REQUESTED.load(Ordering::SeqCst) {
            return Err(anyhow!("Abort requested"));
        }

        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Timeout waiting for rehearsal page after {}ms",
                config.loading_timeout_ms
            ));
        }

        let region_img = capture_region(hwnd, &config.start_button_region)?;
        let current_hist = calculate_histogram(&region_img);
        let similarity = histogram_similarity(&ref_hist, &current_hist);

        crate::log(&format!(
            "Rehearsal page detection: similarity = {:.3} (threshold = {:.3})",
            similarity, config.histogram_threshold
        ));

        if similarity >= config.histogram_threshold {
            crate::log("Start button detected (rehearsal page loaded)");
            return Ok(());
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
