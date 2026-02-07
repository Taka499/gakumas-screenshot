//! Automation runner - main entry point for the automation loop.
//!
//! Coordinates the automation thread and OCR worker thread.
//! Spawns threads, runs the state machine, and handles completion.

use anyhow::{anyhow, Result};
use chrono::Local;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;

use crate::automation::config::get_config;
use crate::automation::csv_writer::init_csv;
use crate::automation::ocr_worker::run_ocr_worker;
use crate::automation::queue::create_work_queue;
use crate::automation::state::{reset_abort_flag, AutomationContext, AutomationState};
use crate::capture::find_gakumas_window;

/// Global flag indicating if automation is currently running.
static AUTOMATION_RUNNING: AtomicBool = AtomicBool::new(false);

/// Current iteration counter (for GUI progress display).
static CURRENT_ITERATION: AtomicU32 = AtomicU32::new(0);

/// Total iterations for current run (for GUI progress display).
static TOTAL_ITERATIONS: AtomicU32 = AtomicU32::new(0);

/// Current state description (for GUI progress display).
static CURRENT_STATE_DESC: Mutex<String> = Mutex::new(String::new());

/// Current session folder path (for GUI to access after completion).
static CURRENT_SESSION_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Default number of iterations if not specified in config.
const DEFAULT_ITERATIONS: u32 = 10;

/// Checks if automation is currently running.
pub fn is_automation_running() -> bool {
    AUTOMATION_RUNNING.load(Ordering::SeqCst)
}

/// Gets the current iteration number (0-based, for GUI progress display).
pub fn get_current_iteration() -> u32 {
    CURRENT_ITERATION.load(Ordering::SeqCst)
}

/// Gets the total number of iterations for current run.
pub fn get_total_iterations() -> u32 {
    TOTAL_ITERATIONS.load(Ordering::SeqCst)
}

/// Gets the current state description (for GUI progress display).
pub fn get_current_state_description() -> String {
    CURRENT_STATE_DESC
        .lock()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "不明".to_string())
}

/// Updates the current state description (called from automation thread).
fn update_state_description(desc: &str) {
    if let Ok(mut s) = CURRENT_STATE_DESC.lock() {
        *s = desc.to_string();
    }
}

/// Gets the current session folder path (for GUI to access).
pub fn get_current_session_path() -> Option<PathBuf> {
    CURRENT_SESSION_PATH
        .lock()
        .ok()
        .and_then(|p| p.clone())
}

/// Sets the current session folder path (called at automation start).
fn set_current_session_path(path: PathBuf) {
    if let Ok(mut p) = CURRENT_SESSION_PATH.lock() {
        *p = Some(path);
    }
}

/// Starts the automation loop in a background thread.
///
/// Returns immediately after spawning the automation thread.
/// Use `is_automation_running()` to check if automation is still active.
///
/// # Arguments
/// * `max_iterations` - Number of iterations to run (uses config default if None)
///
/// # Errors
/// Returns an error if:
/// - Automation is already running
/// - Game window cannot be found
pub fn start_automation(max_iterations: Option<u32>) -> Result<()> {
    // Check if already running
    if AUTOMATION_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(anyhow!("Automation is already running"));
    }

    // Reset abort flag
    reset_abort_flag();

    // Find game window
    let hwnd = match find_gakumas_window() {
        Ok(hwnd) => hwnd,
        Err(e) => {
            AUTOMATION_RUNNING.store(false, Ordering::SeqCst);
            return Err(anyhow!("Failed to find game window: {}", e));
        }
    };

    let config = get_config().clone();
    let iterations = max_iterations.unwrap_or(DEFAULT_ITERATIONS);

    // Create timestamped session folder: output/YYYYMMDD_HHMMSS/
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let session_dir = crate::paths::get_output_dir().join(&timestamp);

    // Create session directory structure
    if let Err(e) = fs::create_dir_all(&session_dir) {
        AUTOMATION_RUNNING.store(false, Ordering::SeqCst);
        return Err(anyhow!("Failed to create session directory: {}", e));
    }

    // Store session path for GUI access
    set_current_session_path(session_dir.clone());

    // Activate per-session logging
    let session_log_path = session_dir.join("session.log");
    crate::set_session_log(Some(session_log_path.clone()));

    // Setup paths within session folder
    let screenshot_dir = session_dir.join("screenshots");
    let csv_path = session_dir.join("results.csv");

    // Create screenshot directory if needed
    if let Err(e) = fs::create_dir_all(&screenshot_dir) {
        AUTOMATION_RUNNING.store(false, Ordering::SeqCst);
        return Err(anyhow!("Failed to create screenshot directory: {}", e));
    }

    // Initialize CSV file
    if let Err(e) = init_csv(&csv_path) {
        AUTOMATION_RUNNING.store(false, Ordering::SeqCst);
        return Err(anyhow!("Failed to initialize CSV file: {}", e));
    }

    // Initialize progress counters for GUI
    CURRENT_ITERATION.store(0, Ordering::SeqCst);
    TOTAL_ITERATIONS.store(iterations, Ordering::SeqCst);
    update_state_description("開始中...");

    crate::log(&format!(
        "Starting automation: {} iterations (Ctrl+Shift+Q to abort)",
        iterations
    ));
    crate::log(&format!("Session folder: {}", session_dir.display()));
    crate::log(&format!("Screenshots: {}", screenshot_dir.display()));
    crate::log(&format!("Results CSV: {}", csv_path.display()));

    // Extract raw pointer value to pass across thread boundary
    // SAFETY: HWND is just a pointer wrapper, and Windows handles are valid
    // across threads. We reconstruct it in the spawned thread.
    let hwnd_raw = hwnd.0 as usize;

    // Spawn automation thread
    thread::spawn(move || {
        // Reconstruct HWND from raw pointer value
        let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
        run_automation_loop(hwnd, config, iterations, screenshot_dir, csv_path);
        AUTOMATION_RUNNING.store(false, Ordering::SeqCst);
        crate::log("Automation thread finished");
    });

    Ok(())
}

/// Runs the automation loop (called from the automation thread).
fn run_automation_loop(
    hwnd: windows::Win32::Foundation::HWND,
    config: crate::automation::config::AutomationConfig,
    max_iterations: u32,
    screenshot_dir: PathBuf,
    csv_path: PathBuf,
) {
    // Create work queue
    let (sender, receiver) = create_work_queue();

    // Spawn OCR worker thread
    let ocr_threshold = config.ocr_threshold;
    let score_regions = config.score_regions;
    let csv_path_clone = csv_path.clone();
    let ocr_handle = thread::spawn(move || {
        run_ocr_worker(receiver, csv_path_clone, ocr_threshold, score_regions);
    });

    // Create and run state machine
    let mut ctx = AutomationContext::new(hwnd, config, max_iterations, sender, screenshot_dir);

    // Run state machine until complete
    loop {
        // Update progress counters for GUI
        CURRENT_ITERATION.store(ctx.current_iteration, Ordering::SeqCst);
        update_state_description(&ctx.state.description_ja());

        match ctx.step() {
            Ok(true) => {
                // Continue running
            }
            Ok(false) => {
                // Complete, error, or aborted
                break;
            }
            Err(e) => {
                crate::log(&format!("Automation error: {}", e));
                break;
            }
        }
    }

    // Log final state
    match &ctx.state {
        AutomationState::Complete => {
            crate::log(&format!(
                "Automation completed successfully: {} iterations",
                max_iterations
            ));
        }
        AutomationState::Aborted => {
            crate::log(&format!(
                "Automation aborted at iteration {}/{}",
                ctx.current_iteration, max_iterations
            ));
        }
        AutomationState::Error(msg) => {
            crate::log(&format!("Automation failed: {}", msg));
        }
        _ => {}
    }

    // Drop the sender to signal OCR worker to finish
    drop(ctx.work_sender);

    // Wait for OCR worker to finish processing remaining items
    crate::log("Waiting for OCR worker to finish...");
    if let Err(e) = ocr_handle.join() {
        crate::log(&format!("OCR worker thread panicked: {:?}", e));
    }

    crate::log("All processing complete");

    // Deactivate per-session logging
    crate::set_session_log(None);
}

/// Re-export request_abort for convenience.
pub use crate::automation::state::request_abort;
