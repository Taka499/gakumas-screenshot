//! Calibration state tracking.
//!
//! Tracks which items have been calibrated and the current step in the wizard.

use crate::automation::{ButtonConfig, RelativeRect};
use windows::Win32::Foundation::HWND;

/// Current state of the calibration process.
pub struct CalibrationState {
    /// Handle to the game window being calibrated.
    pub hwnd: HWND,
    /// Width of the client area in pixels.
    pub client_width: u32,
    /// Height of the client area in pixels.
    pub client_height: u32,
    /// Collected calibration items.
    pub items: CalibrationItems,
    /// Current step in the calibration process.
    pub current_step: CalibrationStep,
}

/// Collected calibration data.
#[derive(Clone, Default)]
pub struct CalibrationItems {
    /// Position of the Start button.
    pub start_button: Option<ButtonConfig>,
    /// Position of the Skip button.
    pub skip_button: Option<ButtonConfig>,
    /// Region for skip button brightness detection.
    pub skip_button_region: Option<RelativeRect>,
    /// Score regions: [stage][character], 3 stages × 3 characters.
    pub score_regions: [[Option<RelativeRect>; 3]; 3],
    /// Stage total regions: one per stage.
    pub stage_total_regions: [Option<RelativeRect>; 3],
}

/// Steps in the calibration wizard.
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
    /// Capture the top-left corner of a score region.
    ScoreRegionTopLeft { stage: usize, character: usize },
    /// Capture the bottom-right corner of a score region.
    ScoreRegionBottomRight { stage: usize, character: usize },
    /// Capture the top-left corner of a stage total region.
    StageTotalRegionTopLeft { stage: usize },
    /// Capture the bottom-right corner of a stage total region.
    StageTotalRegionBottomRight { stage: usize },
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
            Self::ScoreRegionTopLeft { stage, character } => {
                format!(
                    "Score Region S{}C{} - TOP-LEFT corner",
                    stage + 1,
                    character + 1
                )
            }
            Self::ScoreRegionBottomRight { stage, character } => {
                format!(
                    "Score Region S{}C{} - BOTTOM-RIGHT corner",
                    stage + 1,
                    character + 1
                )
            }
            Self::StageTotalRegionTopLeft { stage } => {
                format!("Stage {} Total Region - TOP-LEFT corner", stage + 1)
            }
            Self::StageTotalRegionBottomRight { stage } => {
                format!("Stage {} Total Region - BOTTOM-RIGHT corner", stage + 1)
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
            Self::ScoreRegionTopLeft { stage, character } => 5 + (stage * 3 + character) * 2,
            Self::ScoreRegionBottomRight { stage, character } => 6 + (stage * 3 + character) * 2,
            Self::StageTotalRegionTopLeft { stage } => 23 + stage * 2,
            Self::StageTotalRegionBottomRight { stage } => 24 + stage * 2,
            Self::Complete => 29,
        }
    }

    /// Total number of steps in the wizard.
    pub fn total_steps() -> usize {
        // 2 buttons + 2 for brightness region + 18 for score regions (9 * 2) + 6 for stage totals (3 * 2)
        28
    }
}
