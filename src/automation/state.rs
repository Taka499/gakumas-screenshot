//! Automation state machine for rehearsal data collection.
//!
//! The state machine sequences through: Start → Wait → Skip → Capture → Loop
//! Each state transition checks for abort signals and window validity.

use anyhow::{anyhow, Result};
use chrono::Local;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow};

use crate::automation::config::AutomationConfig;
use crate::automation::detection::{wait_for_loading, wait_for_result, wait_for_start_page};
use crate::automation::input::click_at_relative;
use crate::automation::queue::OcrWorkItem;
use crate::capture::capture_gakumas_to_buffer;

/// Global abort flag - set by abort hotkey handler.
pub static ABORT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Automation state machine states.
#[derive(Debug, Clone, PartialEq)]
pub enum AutomationState {
    /// Waiting to start (initial state)
    Idle,
    /// Waiting for rehearsal start page to appear
    WaitingForStartPage,
    /// Clicking the Start button
    ClickingStart,
    /// Waiting for loading to complete
    WaitingForLoading,
    /// Clicking the Skip button
    ClickingSkip,
    /// Waiting for result screen to appear
    WaitingForResult,
    /// Capturing the result screenshot
    Capturing,
    /// Clicking the End button to return to rehearsal page
    ClickingEnd,
    /// Checking if we should continue or stop
    CheckingLoop,
    /// All iterations complete
    Complete,
    /// Error occurred
    Error(String),
    /// User requested abort
    Aborted,
}

impl std::fmt::Display for AutomationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutomationState::Idle => write!(f, "Idle"),
            AutomationState::WaitingForStartPage => write!(f, "Waiting for start page"),
            AutomationState::ClickingStart => write!(f, "Clicking Start"),
            AutomationState::WaitingForLoading => write!(f, "Waiting for loading"),
            AutomationState::ClickingSkip => write!(f, "Clicking Skip"),
            AutomationState::WaitingForResult => write!(f, "Waiting for result"),
            AutomationState::Capturing => write!(f, "Capturing"),
            AutomationState::ClickingEnd => write!(f, "Clicking End"),
            AutomationState::CheckingLoop => write!(f, "Checking loop"),
            AutomationState::Complete => write!(f, "Complete"),
            AutomationState::Error(msg) => write!(f, "Error: {}", msg),
            AutomationState::Aborted => write!(f, "Aborted"),
        }
    }
}

/// Automation context holding state and configuration.
pub struct AutomationContext {
    /// Current state
    pub state: AutomationState,
    /// Game window handle
    pub hwnd: HWND,
    /// Automation configuration
    pub config: AutomationConfig,
    /// Current iteration number (1-based)
    pub current_iteration: u32,
    /// Maximum number of iterations
    pub max_iterations: u32,
    /// Channel sender for OCR work items
    pub work_sender: Sender<OcrWorkItem>,
    /// Time when automation started
    pub start_time: Instant,
    /// Directory for saving screenshots
    pub screenshot_dir: PathBuf,
}

impl AutomationContext {
    /// Creates a new automation context.
    pub fn new(
        hwnd: HWND,
        config: AutomationConfig,
        max_iterations: u32,
        work_sender: Sender<OcrWorkItem>,
        screenshot_dir: PathBuf,
    ) -> Self {
        Self {
            state: AutomationState::Idle,
            hwnd,
            config,
            current_iteration: 0,
            max_iterations,
            work_sender,
            start_time: Instant::now(),
            screenshot_dir,
        }
    }

    /// Advances the state machine by one step.
    ///
    /// Returns `Ok(true)` if automation should continue, `Ok(false)` if complete/error/aborted.
    pub fn step(&mut self) -> Result<bool> {
        // Check for abort before each state transition
        if ABORT_REQUESTED.load(Ordering::SeqCst) {
            crate::log("Abort requested, stopping automation");
            self.state = AutomationState::Aborted;
            return Ok(false);
        }

        // Check if window is still valid
        if !is_window_valid(self.hwnd) {
            crate::log("Game window no longer exists, aborting");
            self.state = AutomationState::Error("Game window closed".to_string());
            return Ok(false);
        }

        match &self.state {
            AutomationState::Idle => {
                self.current_iteration = 1;
                crate::log(&format!(
                    "Starting automation: {} iterations",
                    self.max_iterations
                ));
                self.state = AutomationState::WaitingForStartPage;
                Ok(true)
            }

            AutomationState::WaitingForStartPage => {
                crate::log(&format!(
                    "Iteration {}/{}: Waiting for rehearsal page...",
                    self.current_iteration, self.max_iterations
                ));

                match wait_for_start_page(self.hwnd, &self.config) {
                    Ok(()) => {
                        self.state = AutomationState::ClickingStart;
                        Ok(true)
                    }
                    Err(e) => {
                        // Check if this was an abort request
                        if ABORT_REQUESTED.load(Ordering::SeqCst) {
                            crate::log("Abort requested during start page wait");
                            self.state = AutomationState::Aborted;
                        } else {
                            self.state =
                                AutomationState::Error(format!("Start page wait failed: {}", e));
                        }
                        Ok(false)
                    }
                }
            }

            AutomationState::ClickingStart => {
                crate::log(&format!(
                    "Iteration {}/{}: Clicking Start button",
                    self.current_iteration, self.max_iterations
                ));

                if let Err(e) = click_with_focus(
                    self.hwnd,
                    self.config.start_button.x,
                    self.config.start_button.y,
                ) {
                    self.state = AutomationState::Error(format!("Failed to click Start: {}", e));
                    return Ok(false);
                }

                self.state = AutomationState::WaitingForLoading;
                Ok(true)
            }

            AutomationState::WaitingForLoading => {
                crate::log(&format!(
                    "Iteration {}/{}: Waiting for loading...",
                    self.current_iteration, self.max_iterations
                ));

                match wait_for_loading(self.hwnd, &self.config) {
                    Ok(()) => {
                        self.state = AutomationState::ClickingSkip;
                        Ok(true)
                    }
                    Err(e) => {
                        // Check if this was an abort request
                        if ABORT_REQUESTED.load(Ordering::SeqCst) {
                            crate::log("Abort requested during loading wait");
                            self.state = AutomationState::Aborted;
                        } else {
                            self.state =
                                AutomationState::Error(format!("Loading wait failed: {}", e));
                        }
                        Ok(false)
                    }
                }
            }

            AutomationState::ClickingSkip => {
                crate::log(&format!(
                    "Iteration {}/{}: Clicking Skip button",
                    self.current_iteration, self.max_iterations
                ));

                if let Err(e) = click_with_focus(
                    self.hwnd,
                    self.config.skip_button.x,
                    self.config.skip_button.y,
                ) {
                    self.state = AutomationState::Error(format!("Failed to click Skip: {}", e));
                    return Ok(false);
                }

                self.state = AutomationState::WaitingForResult;
                Ok(true)
            }

            AutomationState::WaitingForResult => {
                crate::log(&format!(
                    "Iteration {}/{}: Waiting for result screen...",
                    self.current_iteration, self.max_iterations
                ));

                match wait_for_result(self.hwnd, &self.config) {
                    Ok(()) => {
                        self.state = AutomationState::Capturing;
                        Ok(true)
                    }
                    Err(e) => {
                        // Check if this was an abort request
                        if ABORT_REQUESTED.load(Ordering::SeqCst) {
                            crate::log("Abort requested during result wait");
                            self.state = AutomationState::Aborted;
                        } else {
                            self.state =
                                AutomationState::Error(format!("Result wait failed: {}", e));
                        }
                        Ok(false)
                    }
                }
            }

            AutomationState::Capturing => {
                crate::log(&format!(
                    "Iteration {}/{}: Capturing screenshot",
                    self.current_iteration, self.max_iterations
                ));

                // Capture screenshot
                let img = match capture_gakumas_to_buffer(self.hwnd) {
                    Ok(img) => img,
                    Err(e) => {
                        self.state =
                            AutomationState::Error(format!("Failed to capture: {}", e));
                        return Ok(false);
                    }
                };

                // Generate filename with timestamp
                let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                let filename = format!("{:03}_{}.png", self.current_iteration, timestamp);
                let screenshot_path = self.screenshot_dir.join(&filename);

                // Save screenshot
                if let Err(e) = img.save(&screenshot_path) {
                    self.state =
                        AutomationState::Error(format!("Failed to save screenshot: {}", e));
                    return Ok(false);
                }

                crate::log(&format!(
                    "Iteration {}/{}: Screenshot saved to {}",
                    self.current_iteration,
                    self.max_iterations,
                    screenshot_path.display()
                ));

                // Queue for OCR processing
                let work_item = OcrWorkItem::new(screenshot_path, self.current_iteration);
                if let Err(e) = self.work_sender.send(work_item) {
                    crate::log(&format!("Warning: Failed to queue OCR work item: {}", e));
                    // Don't fail automation for this - OCR is secondary
                }

                self.state = AutomationState::ClickingEnd;
                Ok(true)
            }

            AutomationState::ClickingEnd => {
                crate::log(&format!(
                    "Iteration {}/{}: Clicking End button",
                    self.current_iteration, self.max_iterations
                ));

                if let Err(e) = click_with_focus(
                    self.hwnd,
                    self.config.end_button.x,
                    self.config.end_button.y,
                ) {
                    self.state = AutomationState::Error(format!("Failed to click End: {}", e));
                    return Ok(false);
                }

                // Small delay to let the page transition start
                std::thread::sleep(Duration::from_millis(500));

                self.state = AutomationState::CheckingLoop;
                Ok(true)
            }

            AutomationState::CheckingLoop => {
                if self.current_iteration >= self.max_iterations {
                    crate::log(&format!(
                        "Automation complete: {} iterations in {:.1}s",
                        self.max_iterations,
                        self.start_time.elapsed().as_secs_f32()
                    ));
                    self.state = AutomationState::Complete;
                    Ok(false)
                } else {
                    self.current_iteration += 1;
                    // Wait for start page before clicking Start again
                    self.state = AutomationState::WaitingForStartPage;
                    Ok(true)
                }
            }

            AutomationState::Complete | AutomationState::Error(_) | AutomationState::Aborted => {
                Ok(false)
            }
        }
    }

    /// Returns a progress string for display (e.g., in tray tooltip).
    pub fn progress_string(&self) -> String {
        match &self.state {
            AutomationState::Complete => {
                format!("Complete ({} iterations)", self.max_iterations)
            }
            AutomationState::Error(msg) => format!("Error: {}", msg),
            AutomationState::Aborted => "Aborted".to_string(),
            _ => format!("{}/{} - {}", self.current_iteration, self.max_iterations, self.state),
        }
    }
}

/// Checks if a window handle is still valid.
fn is_window_valid(hwnd: HWND) -> bool {
    unsafe { IsWindow(hwnd).as_bool() }
}

/// Clicks at a relative position after re-focusing the window.
///
/// Re-focusing is important because the user might click elsewhere during automation.
fn click_with_focus(hwnd: HWND, rel_x: f32, rel_y: f32) -> Result<()> {
    if !is_window_valid(hwnd) {
        return Err(anyhow!("Game window no longer exists"));
    }

    // Bring window to foreground
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    std::thread::sleep(Duration::from_millis(50));

    click_at_relative(hwnd, rel_x, rel_y)
}

/// Resets the abort flag. Call before starting automation.
pub fn reset_abort_flag() {
    ABORT_REQUESTED.store(false, Ordering::SeqCst);
}

/// Requests abort of running automation.
pub fn request_abort() {
    ABORT_REQUESTED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", AutomationState::Idle), "Idle");
        assert_eq!(
            format!("{}", AutomationState::WaitingForLoading),
            "Waiting for loading"
        );
        assert_eq!(
            format!("{}", AutomationState::Error("test".to_string())),
            "Error: test"
        );
    }
}
