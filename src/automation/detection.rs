//! Loading state detection via brightness analysis.
//!
//! This module provides functions to detect when the game finishes loading
//! by monitoring the brightness of screen regions.

use anyhow::{anyhow, Result};
use image::{ImageBuffer, Rgba};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;

use crate::automation::config::AutomationConfig;
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

/// Waits until the skip button region brightness exceeds the threshold.
///
/// Polls repeatedly, capturing the skip_button_region and measuring brightness.
/// Returns Ok(()) when brightness exceeds threshold, or Err on timeout.
pub fn wait_for_loading(hwnd: HWND, config: &AutomationConfig) -> Result<()> {
    let start = Instant::now();
    let timeout = Duration::from_millis(config.loading_timeout_ms);

    loop {
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "Loading timeout after {}ms",
                config.loading_timeout_ms
            ));
        }

        let region_img = capture_region(hwnd, &config.skip_button_region)?;
        let brightness = calculate_brightness(&region_img);

        crate::log(&format!("Loading check: brightness = {:.2}", brightness));

        if brightness > config.brightness_threshold {
            crate::log("Loading complete (brightness threshold exceeded)");
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
