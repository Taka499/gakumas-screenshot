//! GUI application state management.
//!
//! Tracks user input values and automation status for display.

use std::path::PathBuf;
use std::time::Instant;

/// Automation status for display in GUI.
#[derive(Clone, Debug)]
pub enum AutomationStatus {
    /// Not running, ready to start
    Idle,
    /// Automation is running
    Running {
        current: u32,
        total: u32,
        state_description: String,
        start_time: Instant,
    },
    /// Automation completed successfully
    Completed {
        total: u32,
        session_path: PathBuf,
    },
    /// Automation was aborted by user
    Aborted,
    /// Automation failed with error
    Error(String),
}

impl Default for AutomationStatus {
    fn default() -> Self {
        Self::Idle
    }
}

impl AutomationStatus {
    /// Get display text for current status.
    pub fn status_text(&self) -> String {
        match self {
            Self::Idle => "待機中".to_string(),
            Self::Running { current, total, state_description, .. } => {
                format!("実行中 ({}/{}) - {}", current, total, state_description)
            }
            Self::Completed { total, .. } => {
                format!("完了 ({}回)", total)
            }
            Self::Aborted => "中断".to_string(),
            Self::Error(msg) => format!("エラー: {}", msg),
        }
    }

    /// Get progress as percentage (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        match self {
            Self::Running { current, total, .. } if *total > 0 => {
                *current as f32 / *total as f32
            }
            Self::Completed { .. } => 1.0,
            _ => 0.0,
        }
    }

    /// Get elapsed time string if running.
    pub fn elapsed_text(&self) -> Option<String> {
        match self {
            Self::Running { start_time, .. } => {
                let elapsed = start_time.elapsed();
                let secs = elapsed.as_secs();
                let mins = secs / 60;
                let secs = secs % 60;
                Some(format!("{:02}:{:02}", mins, secs))
            }
            _ => None,
        }
    }

    /// Check if automation is currently running.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// GUI application state.
#[derive(Debug)]
pub struct GuiState {
    /// Number of iterations to run (user input).
    pub iterations: u32,
    /// Current automation status.
    pub status: AutomationStatus,
    /// Path to the latest session folder (for "Open Folder" button).
    pub latest_session_path: Option<PathBuf>,
    /// Start time of current automation run.
    pub automation_start_time: Option<Instant>,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            iterations: 100,
            status: AutomationStatus::Idle,
            latest_session_path: None,
            automation_start_time: None,
        }
    }
}
