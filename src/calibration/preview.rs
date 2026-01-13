//! Preview rendering for calibration visualization.
//!
//! Draws rectangles, crosshairs, and labels on screenshots to show
//! configured regions and button positions.

use anyhow::Result;
use image::{ImageBuffer, Rgba};
use std::process::Command;

use crate::automation::AutomationConfig;

/// Color constants for preview rendering.
pub const COLOR_SCORE_REGION: Rgba<u8> = Rgba([0, 255, 0, 255]); // Green
pub const COLOR_TOTAL_REGION: Rgba<u8> = Rgba([0, 0, 255, 255]); // Blue
pub const COLOR_BUTTON: Rgba<u8> = Rgba([255, 0, 0, 255]); // Red
pub const COLOR_BRIGHTNESS: Rgba<u8> = Rgba([255, 255, 0, 255]); // Yellow
pub const COLOR_HIGHLIGHT: Rgba<u8> = Rgba([255, 128, 0, 255]); // Orange

/// What item to highlight in the preview.
#[derive(Clone, Debug)]
pub enum HighlightedItem {
    StartButton,
    SkipButton,
    SkipButtonRegion,
    ScoreRegion { stage: usize, character: usize },
    StageTotalRegion { stage: usize },
}

/// Renders all configured regions onto a screenshot.
pub fn render_preview(
    screenshot: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    config: &AutomationConfig,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut img = screenshot.clone();
    let (width, height) = img.dimensions();

    // Draw buttons as crosshairs
    draw_crosshair(
        &mut img,
        (config.start_button.x * width as f32) as u32,
        (config.start_button.y * height as f32) as u32,
        COLOR_BUTTON,
        15,
    );
    draw_crosshair(
        &mut img,
        (config.skip_button.x * width as f32) as u32,
        (config.skip_button.y * height as f32) as u32,
        COLOR_BUTTON,
        15,
    );

    // Draw skip button brightness region
    let r = &config.skip_button_region;
    draw_rect(
        &mut img,
        (r.x * width as f32) as u32,
        (r.y * height as f32) as u32,
        (r.width * width as f32) as u32,
        (r.height * height as f32) as u32,
        COLOR_BRIGHTNESS,
        2,
    );

    // Draw score regions if configured
    if let Some(score_regions) = &config.score_regions {
        for (stage, stage_regions) in score_regions.iter().enumerate() {
            for (character, region) in stage_regions.iter().enumerate() {
                draw_rect(
                    &mut img,
                    (region.x * width as f32) as u32,
                    (region.y * height as f32) as u32,
                    (region.width * width as f32) as u32,
                    (region.height * height as f32) as u32,
                    COLOR_SCORE_REGION,
                    2,
                );
                // Label would go here (S1C1, etc.)
                let _ = (stage, character); // suppress unused warnings for now
            }
        }
    }

    // Draw stage total regions if configured
    if let Some(total_regions) = &config.stage_total_regions {
        for (_stage, region) in total_regions.iter().enumerate() {
            draw_rect(
                &mut img,
                (region.x * width as f32) as u32,
                (region.y * height as f32) as u32,
                (region.width * width as f32) as u32,
                (region.height * height as f32) as u32,
                COLOR_TOTAL_REGION,
                2,
            );
        }
    }

    img
}

/// Renders preview with a single highlighted region (for per-step preview).
pub fn render_preview_with_highlight(
    screenshot: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    config: &AutomationConfig,
    highlight: &HighlightedItem,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut img = render_preview(screenshot, config);
    let (width, height) = img.dimensions();

    // Draw the highlighted item with a thicker orange border
    match highlight {
        HighlightedItem::StartButton => {
            draw_crosshair(
                &mut img,
                (config.start_button.x * width as f32) as u32,
                (config.start_button.y * height as f32) as u32,
                COLOR_HIGHLIGHT,
                20,
            );
        }
        HighlightedItem::SkipButton => {
            draw_crosshair(
                &mut img,
                (config.skip_button.x * width as f32) as u32,
                (config.skip_button.y * height as f32) as u32,
                COLOR_HIGHLIGHT,
                20,
            );
        }
        HighlightedItem::SkipButtonRegion => {
            let r = &config.skip_button_region;
            draw_rect(
                &mut img,
                (r.x * width as f32) as u32,
                (r.y * height as f32) as u32,
                (r.width * width as f32) as u32,
                (r.height * height as f32) as u32,
                COLOR_HIGHLIGHT,
                4,
            );
        }
        HighlightedItem::ScoreRegion { stage, character } => {
            if let Some(score_regions) = &config.score_regions {
                let r = &score_regions[*stage][*character];
                draw_rect(
                    &mut img,
                    (r.x * width as f32) as u32,
                    (r.y * height as f32) as u32,
                    (r.width * width as f32) as u32,
                    (r.height * height as f32) as u32,
                    COLOR_HIGHLIGHT,
                    4,
                );
            }
        }
        HighlightedItem::StageTotalRegion { stage } => {
            if let Some(total_regions) = &config.stage_total_regions {
                let r = &total_regions[*stage];
                draw_rect(
                    &mut img,
                    (r.x * width as f32) as u32,
                    (r.y * height as f32) as u32,
                    (r.width * width as f32) as u32,
                    (r.height * height as f32) as u32,
                    COLOR_HIGHLIGHT,
                    4,
                );
            }
        }
    }

    img
}

/// Saves preview image and opens with system default viewer.
pub fn show_preview(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, filename: &str) -> Result<()> {
    // Save to file
    img.save(filename)?;

    // Open with default viewer on Windows
    Command::new("cmd")
        .args(["/C", "start", "", filename])
        .spawn()?;

    Ok(())
}

/// Draws a rectangle border on an image.
pub fn draw_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: Rgba<u8>,
    thickness: u32,
) {
    let (img_w, img_h) = img.dimensions();

    // Top edge
    for dy in 0..thickness {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                img.put_pixel(px, py, color);
            }
        }
    }

    // Bottom edge
    for dy in 0..thickness {
        for dx in 0..w {
            let px = x + dx;
            let py = y + h.saturating_sub(1) - dy;
            if px < img_w && py < img_h {
                img.put_pixel(px, py, color);
            }
        }
    }

    // Left edge
    for dy in 0..h {
        for dx in 0..thickness {
            let px = x + dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                img.put_pixel(px, py, color);
            }
        }
    }

    // Right edge
    for dy in 0..h {
        for dx in 0..thickness {
            let px = x + w.saturating_sub(1) - dx;
            let py = y + dy;
            if px < img_w && py < img_h {
                img.put_pixel(px, py, color);
            }
        }
    }
}

/// Draws a crosshair at a point.
pub fn draw_crosshair(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: u32,
    y: u32,
    color: Rgba<u8>,
    arm_length: u32,
) {
    let (img_w, img_h) = img.dimensions();

    // Horizontal line
    for dx in 0..=arm_length * 2 {
        let px = (x as i32 - arm_length as i32 + dx as i32) as u32;
        if px < img_w && y < img_h {
            img.put_pixel(px, y, color);
            // Make it thicker
            if y > 0 {
                img.put_pixel(px, y - 1, color);
            }
            if y + 1 < img_h {
                img.put_pixel(px, y + 1, color);
            }
        }
    }

    // Vertical line
    for dy in 0..=arm_length * 2 {
        let py = (y as i32 - arm_length as i32 + dy as i32) as u32;
        if x < img_w && py < img_h {
            img.put_pixel(x, py, color);
            // Make it thicker
            if x > 0 {
                img.put_pixel(x - 1, py, color);
            }
            if x + 1 < img_w {
                img.put_pixel(x + 1, py, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_rect() {
        let mut img = ImageBuffer::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        draw_rect(&mut img, 10, 10, 50, 30, COLOR_SCORE_REGION, 2);

        // Check top-left corner is green
        assert_eq!(*img.get_pixel(10, 10), COLOR_SCORE_REGION);
        // Check center is still black
        assert_eq!(*img.get_pixel(35, 25), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn test_draw_crosshair() {
        let mut img = ImageBuffer::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
        draw_crosshair(&mut img, 50, 50, COLOR_BUTTON, 10);

        // Check center is red
        assert_eq!(*img.get_pixel(50, 50), COLOR_BUTTON);
        // Check arm is red
        assert_eq!(*img.get_pixel(60, 50), COLOR_BUTTON);
    }
}
