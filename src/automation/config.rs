//! Configuration types for automation.
//!
//! Loads settings from config.json at startup. Provides button positions,
//! detection thresholds, and timing parameters.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// Global configuration instance, initialized once at startup.
static CONFIG: OnceLock<AutomationConfig> = OnceLock::new();

/// A rectangle in relative coordinates (0.0 to 1.0).
/// Used for defining screen regions that scale with window size.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RelativeRect {
    /// X position of top-left corner (0.0 = left edge, 1.0 = right edge)
    pub x: f32,
    /// Y position of top-left corner (0.0 = top edge, 1.0 = bottom edge)
    pub y: f32,
    /// Width as fraction of window width
    pub width: f32,
    /// Height as fraction of window height
    pub height: f32,
}

impl Default for RelativeRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.1,
            height: 0.1,
        }
    }
}

/// A point in relative coordinates for button centers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ButtonConfig {
    /// X position (0.0 = left edge, 1.0 = right edge)
    pub x: f32,
    /// Y position (0.0 = top edge, 1.0 = bottom edge)
    pub y: f32,
}

impl Default for ButtonConfig {
    fn default() -> Self {
        Self { x: 0.5, y: 0.5 }
    }
}

/// Complete automation configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutomationConfig {
    /// Position of the "開始する" (Start) button
    pub start_button: ButtonConfig,
    /// Position of the "スキップ" (Skip) button
    pub skip_button: ButtonConfig,
    /// Region around skip button for brightness detection
    pub skip_button_region: RelativeRect,
    /// Brightness threshold: above this = Skip button enabled, below = disabled/dimmed
    pub brightness_threshold: f32,
    /// Histogram similarity threshold: above this = Skip button detected (0.0-1.0)
    #[serde(default = "default_histogram_threshold")]
    pub histogram_threshold: f32,
    /// Path to Skip button reference image for histogram comparison
    #[serde(default = "default_skip_button_reference")]
    pub skip_button_reference: String,
    /// Maximum time to wait for loading (milliseconds)
    pub loading_timeout_ms: u64,
    /// Delay after clicking skip before capturing result (milliseconds)
    pub capture_delay_ms: u64,
    /// Test position for relative click hotkey
    pub test_click_position: ButtonConfig,
    /// OCR brightness threshold (pixels with R, G, B all > threshold are kept)
    #[serde(default = "default_ocr_threshold")]
    pub ocr_threshold: u8,
}

fn default_ocr_threshold() -> u8 {
    190
}

fn default_histogram_threshold() -> f32 {
    0.85 // 85% similarity required to detect Skip button
}

fn default_skip_button_reference() -> String {
    "skip_button_ref.png".to_string()
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            start_button: ButtonConfig { x: 0.5, y: 0.85 },
            skip_button: ButtonConfig { x: 0.9, y: 0.95 },
            skip_button_region: RelativeRect {
                x: 0.85,
                y: 0.90,
                width: 0.10,
                height: 0.10,
            },
            // Brightness threshold: Skip button dimmed ~92, enabled ~97
            // Set to 95 to detect when Skip button becomes enabled
            brightness_threshold: 95.0,
            histogram_threshold: default_histogram_threshold(),
            skip_button_reference: default_skip_button_reference(),
            loading_timeout_ms: 30000,
            capture_delay_ms: 500,
            test_click_position: ButtonConfig { x: 0.5, y: 0.5 },
            ocr_threshold: default_ocr_threshold(),
        }
    }
}

/// Loads configuration from config.json or returns defaults.
/// Looks for config.json in the same directory as the executable.
fn load_config() -> AutomationConfig {
    // Try to find config.json next to the executable
    let config_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("config.json")))
        .unwrap_or_else(|| Path::new("config.json").to_path_buf());

    crate::log(&format!("Looking for config at: {}", config_path.display()));

    if config_path.exists() {
        match fs::read_to_string(config_path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(config) => {
                    crate::log("Config loaded from config.json");
                    return config;
                }
                Err(e) => {
                    crate::log(&format!(
                        "Failed to parse config.json: {}. Using defaults.",
                        e
                    ));
                }
            },
            Err(e) => {
                crate::log(&format!(
                    "Failed to read config.json: {}. Using defaults.",
                    e
                ));
            }
        }
    } else {
        crate::log("config.json not found. Using default config.");
    }

    AutomationConfig::default()
}

/// Initializes the global configuration. Call once at startup.
pub fn init_config() {
    let _ = CONFIG.set(load_config());
}

/// Returns a reference to the global configuration.
/// Panics if called before init_config().
pub fn get_config() -> &'static AutomationConfig {
    CONFIG
        .get()
        .expect("Config not initialized. Call init_config() first.")
}
