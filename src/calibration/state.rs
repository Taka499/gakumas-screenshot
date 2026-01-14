//! Calibration state tracking.
//!
//! Tracks which items have been calibrated and the current step in the wizard.
//!
//! Note: Score regions are no longer needed for calibration. The OCR module
//! uses full-image processing with pattern matching instead of region-based extraction.

use crate::automation::{ButtonConfig, RelativeRect};

/// Collected calibration data.
#[derive(Clone, Default)]
pub struct CalibrationItems {
    /// Position of the Start button.
    pub start_button: Option<ButtonConfig>,
    /// Position of the Skip button.
    pub skip_button: Option<ButtonConfig>,
    /// Region for skip button brightness detection.
    pub skip_button_region: Option<RelativeRect>,
}

/// Steps in the calibration wizard.
///
/// The wizard now only collects 3 items:
/// 1. Start button position (for automation)
/// 2. Skip button position (for automation)
/// 3. Skip button region (for loading detection)
#[derive(Clone, Debug, PartialEq)]
pub enum CalibrationStep {
    /// Capture the Start button position.
    StartButton,
    /// Capture the Skip button position.
    SkipButton,
    /// Capture the top-left corner of skip button brightness region.
    SkipButtonRegionTopLeft,
    /// Capture the bottom-right corner of skip button brightness region.
    SkipButtonRegionBottomRight,
    /// Calibration complete.
    Complete,
}

impl CalibrationStep {
    /// Returns a human-readable description of the current step.
    pub fn description(&self) -> String {
        match self {
            Self::StartButton => "Start Button (開始する)".to_string(),
            Self::SkipButton => "Skip Button (スキップ)".to_string(),
            Self::SkipButtonRegionTopLeft => "Skip Button Region - TOP-LEFT corner".to_string(),
            Self::SkipButtonRegionBottomRight => {
                "Skip Button Region - BOTTOM-RIGHT corner".to_string()
            }
            Self::Complete => "Complete".to_string(),
        }
    }

    /// Returns the step number (1-based) for display.
    pub fn step_number(&self) -> usize {
        match self {
            Self::StartButton => 1,
            Self::SkipButton => 2,
            Self::SkipButtonRegionTopLeft => 3,
            Self::SkipButtonRegionBottomRight => 4,
            Self::Complete => 5,
        }
    }

    /// Total number of steps in the wizard.
    pub fn total_steps() -> usize {
        // 2 buttons + 2 for brightness region (TopLeft + BottomRight)
        4
    }
}
