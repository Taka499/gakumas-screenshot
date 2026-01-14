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
    /// Region for start button histogram detection (detecting rehearsal page).
    pub start_button_region: Option<RelativeRect>,
    /// Position of the Skip button.
    pub skip_button: Option<ButtonConfig>,
    /// Region for skip button brightness detection.
    pub skip_button_region: Option<RelativeRect>,
    /// Position of the End button.
    pub end_button: Option<ButtonConfig>,
    /// Region for end button histogram detection (detecting result page).
    pub end_button_region: Option<RelativeRect>,
}

/// Steps in the calibration wizard.
///
/// The wizard collects 9 items:
/// 1. Start button position (for clicking)
/// 2. Start button region (for page detection)
/// 3. Skip button position (for clicking)
/// 4. Skip button region (for loading detection)
/// 5. End button position (for clicking)
/// 6. End button region (for result page detection)
#[derive(Clone, Debug, PartialEq)]
pub enum CalibrationStep {
    /// Capture the Start button position.
    StartButton,
    /// Capture the top-left corner of start button region.
    StartButtonRegionTopLeft,
    /// Capture the bottom-right corner of start button region.
    StartButtonRegionBottomRight,
    /// Capture the Skip button position.
    SkipButton,
    /// Capture the top-left corner of skip button brightness region.
    SkipButtonRegionTopLeft,
    /// Capture the bottom-right corner of skip button brightness region.
    SkipButtonRegionBottomRight,
    /// Capture the End button position.
    EndButton,
    /// Capture the top-left corner of end button region.
    EndButtonRegionTopLeft,
    /// Capture the bottom-right corner of end button region.
    EndButtonRegionBottomRight,
    /// Calibration complete.
    Complete,
}

impl CalibrationStep {
    /// Returns a human-readable description of the current step.
    pub fn description(&self) -> String {
        match self {
            Self::StartButton => "Start Button (開始する) - click position".to_string(),
            Self::StartButtonRegionTopLeft => "Start Button Region - TOP-LEFT corner".to_string(),
            Self::StartButtonRegionBottomRight => {
                "Start Button Region - BOTTOM-RIGHT corner".to_string()
            }
            Self::SkipButton => "Skip Button (スキップ) - click position".to_string(),
            Self::SkipButtonRegionTopLeft => "Skip Button Region - TOP-LEFT corner".to_string(),
            Self::SkipButtonRegionBottomRight => {
                "Skip Button Region - BOTTOM-RIGHT corner".to_string()
            }
            Self::EndButton => "End Button (終了) - click position".to_string(),
            Self::EndButtonRegionTopLeft => "End Button Region - TOP-LEFT corner".to_string(),
            Self::EndButtonRegionBottomRight => {
                "End Button Region - BOTTOM-RIGHT corner".to_string()
            }
            Self::Complete => "Complete".to_string(),
        }
    }

    /// Returns the step number (1-based) for display.
    pub fn step_number(&self) -> usize {
        match self {
            Self::StartButton => 1,
            Self::StartButtonRegionTopLeft => 2,
            Self::StartButtonRegionBottomRight => 3,
            Self::SkipButton => 4,
            Self::SkipButtonRegionTopLeft => 5,
            Self::SkipButtonRegionBottomRight => 6,
            Self::EndButton => 7,
            Self::EndButtonRegionTopLeft => 8,
            Self::EndButtonRegionBottomRight => 9,
            Self::Complete => 10,
        }
    }

    /// Total number of steps in the wizard.
    pub fn total_steps() -> usize {
        // 3 buttons + 3 regions (each with TopLeft + BottomRight)
        9
    }
}
