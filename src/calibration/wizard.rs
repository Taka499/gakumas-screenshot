//! Calibration wizard implementation.
//!
//! Provides interactive calibration using hotkeys and console prompts.

use anyhow::{anyhow, Result};
use std::sync::Mutex;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT, VK_ESCAPE, VK_F1, VK_F2, VK_F3, VK_N,
    VK_RETURN, VK_Y,
};

use crate::automation::{get_config, ButtonConfig, RelativeRect};
use crate::calibration::coords::{get_cursor_position, screen_to_relative};
use crate::calibration::preview::{render_preview, render_preview_with_highlight, show_preview, HighlightedItem};
use crate::calibration::state::{CalibrationItems, CalibrationStep};
use crate::capture::{capture_gakumas_to_buffer, find_gakumas_window};
use crate::log;

// Calibration hotkey IDs (must not conflict with main hotkeys)
pub const HOTKEY_CAL_F1: i32 = 100;
pub const HOTKEY_CAL_F2: i32 = 101;
pub const HOTKEY_CAL_F3: i32 = 102;
pub const HOTKEY_CAL_Y: i32 = 103;
pub const HOTKEY_CAL_N: i32 = 104;
pub const HOTKEY_CAL_ESCAPE: i32 = 105;
pub const HOTKEY_CAL_ENTER: i32 = 106;

/// Global calibration state protected by mutex.
static CALIBRATION: Mutex<Option<CalibrationContext>> = Mutex::new(None);

/// Runtime context for an active calibration session.
/// Note: HWND is stored as isize for Send+Sync safety with Mutex.
struct CalibrationContext {
    /// Handle to the main application window (for hotkey registration).
    app_hwnd: isize,
    /// Handle to the game window being calibrated.
    game_hwnd: isize,
    /// Current step in the calibration process.
    current_step: CalibrationStep,
    /// Collected calibration data.
    items: CalibrationItems,
    /// Temporary storage for region top-left corner.
    pending_top_left: Option<(f32, f32)>,
    /// Whether we're waiting for user confirmation (Y/N).
    awaiting_confirmation: bool,
}

/// Converts an HWND to isize for storage.
fn hwnd_to_isize(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

/// Converts an isize back to HWND.
fn isize_to_hwnd(value: isize) -> HWND {
    HWND(value as *mut std::ffi::c_void)
}

/// Returns true if calibration is currently active.
pub fn is_calibrating() -> bool {
    CALIBRATION.lock().unwrap().is_some()
}

/// Starts the calibration wizard.
pub fn start_calibration(app_hwnd: HWND) -> Result<()> {
    // Check if already calibrating
    if is_calibrating() {
        log("Calibration already in progress.");
        return Ok(());
    }

    // Find game window
    let game_hwnd = find_gakumas_window()?;
    log(&format!("Found game window: {:?}", game_hwnd));

    // Register calibration hotkeys
    register_calibration_hotkeys(app_hwnd)?;

    // Initialize calibration context
    let context = CalibrationContext {
        app_hwnd: hwnd_to_isize(app_hwnd),
        game_hwnd: hwnd_to_isize(game_hwnd),
        current_step: CalibrationStep::StartButton,
        items: CalibrationItems::default(),
        pending_top_left: None,
        awaiting_confirmation: false,
    };

    *CALIBRATION.lock().unwrap() = Some(context);

    // Print welcome message and first step
    log("");
    log("=======================================================");
    log("           CALIBRATION MODE STARTED");
    log("=======================================================");
    log("");
    log("Hotkeys:");
    log("  F1     - Record point (for buttons)");
    log("  F2     - Record top-left corner (for regions)");
    log("  F3     - Record bottom-right corner (for regions)");
    log("  Y      - Confirm current step");
    log("  N      - Redo current step");
    log("  Enter  - Skip step (keep existing value)");
    log("  Escape - Abort calibration");
    log("");

    print_current_step_instructions();

    Ok(())
}

/// Handles a calibration hotkey press.
pub fn handle_calibration_hotkey(hotkey_id: i32) -> Result<()> {
    let mut guard = CALIBRATION.lock().unwrap();
    let ctx = match guard.as_mut() {
        Some(c) => c,
        None => return Ok(()), // Not calibrating
    };

    match hotkey_id {
        HOTKEY_CAL_ESCAPE => {
            log("");
            log("Calibration aborted by user.");
            drop(guard);
            stop_calibration()?;
            return Ok(());
        }
        HOTKEY_CAL_Y => {
            if ctx.awaiting_confirmation {
                ctx.awaiting_confirmation = false;
                advance_to_next_step(ctx)?;
            }
        }
        HOTKEY_CAL_N => {
            if ctx.awaiting_confirmation {
                ctx.awaiting_confirmation = false;
                ctx.pending_top_left = None;
                log("Redoing current step...");
                log("");
                // Go back to the start of the current item (TopLeft for regions)
                ctx.current_step = rewind_to_step_start(&ctx.current_step);
                print_step_instructions(&ctx.current_step);
            }
        }
        HOTKEY_CAL_F1 => {
            if !ctx.awaiting_confirmation {
                handle_point_capture(ctx)?;
            }
        }
        HOTKEY_CAL_F2 => {
            if !ctx.awaiting_confirmation {
                handle_top_left_capture(ctx)?;
            }
        }
        HOTKEY_CAL_F3 => {
            if !ctx.awaiting_confirmation {
                handle_bottom_right_capture(ctx)?;
            }
        }
        HOTKEY_CAL_ENTER => {
            if !ctx.awaiting_confirmation {
                log("Skipping current step (keeping existing value)...");
                log("");
                skip_current_step(ctx)?;
            }
        }
        _ => {}
    }

    // Check if calibration is complete
    if matches!(ctx.current_step, CalibrationStep::Complete) {
        let items = ctx.items.clone();
        let game_hwnd = isize_to_hwnd(ctx.game_hwnd);
        drop(guard);
        finish_calibration(items, game_hwnd)?;
    }

    Ok(())
}

/// Handles F1 press - record a single point (for buttons).
fn handle_point_capture(ctx: &mut CalibrationContext) -> Result<()> {
    // Check if current step expects a point
    let is_button_step = matches!(
        ctx.current_step,
        CalibrationStep::StartButton | CalibrationStep::SkipButton
    );

    if !is_button_step {
        log("Use F2/F3 for region capture, not F1.");
        return Ok(());
    }

    // Get cursor position and convert to relative
    let (screen_x, screen_y) = get_cursor_position()?;
    let game_hwnd = isize_to_hwnd(ctx.game_hwnd);
    let (rel_x, rel_y) = match screen_to_relative(game_hwnd, screen_x, screen_y) {
        Ok(pos) => pos,
        Err(e) => {
            log(&format!("Error: {}", e));
            return Ok(());
        }
    };

    log(&format!("Recorded position: ({:.3}, {:.3})", rel_x, rel_y));

    // Store the position
    let button = ButtonConfig { x: rel_x, y: rel_y };
    match ctx.current_step {
        CalibrationStep::StartButton => ctx.items.start_button = Some(button),
        CalibrationStep::SkipButton => ctx.items.skip_button = Some(button),
        _ => {}
    }

    // Show preview and ask for confirmation
    show_step_preview(ctx)?;
    ctx.awaiting_confirmation = true;
    log("Press Y to confirm, N to redo.");

    Ok(())
}

/// Handles F2 press - record top-left corner of a region.
fn handle_top_left_capture(ctx: &mut CalibrationContext) -> Result<()> {
    // Check if current step expects a top-left corner
    let is_top_left_step = matches!(
        ctx.current_step,
        CalibrationStep::SkipButtonRegionTopLeft
            | CalibrationStep::ScoreRegionTopLeft { .. }
            | CalibrationStep::StageTotalRegionTopLeft { .. }
    );

    if !is_top_left_step {
        log("Current step doesn't expect top-left corner (F2).");
        return Ok(());
    }

    // Get cursor position and convert to relative
    let (screen_x, screen_y) = get_cursor_position()?;
    let game_hwnd = isize_to_hwnd(ctx.game_hwnd);
    let (rel_x, rel_y) = match screen_to_relative(game_hwnd, screen_x, screen_y) {
        Ok(pos) => pos,
        Err(e) => {
            log(&format!("Error: {}", e));
            return Ok(());
        }
    };

    log(&format!("Top-left recorded: ({:.3}, {:.3})", rel_x, rel_y));
    ctx.pending_top_left = Some((rel_x, rel_y));

    // Advance to bottom-right step
    ctx.current_step = match ctx.current_step {
        CalibrationStep::SkipButtonRegionTopLeft => CalibrationStep::SkipButtonRegionBottomRight,
        CalibrationStep::ScoreRegionTopLeft { stage, character } => {
            CalibrationStep::ScoreRegionBottomRight { stage, character }
        }
        CalibrationStep::StageTotalRegionTopLeft { stage } => {
            CalibrationStep::StageTotalRegionBottomRight { stage }
        }
        _ => ctx.current_step.clone(),
    };

    log("Now position cursor at BOTTOM-RIGHT corner and press F3.");

    Ok(())
}

/// Handles F3 press - record bottom-right corner of a region.
fn handle_bottom_right_capture(ctx: &mut CalibrationContext) -> Result<()> {
    // Check if current step expects a bottom-right corner
    let is_bottom_right_step = matches!(
        ctx.current_step,
        CalibrationStep::SkipButtonRegionBottomRight
            | CalibrationStep::ScoreRegionBottomRight { .. }
            | CalibrationStep::StageTotalRegionBottomRight { .. }
    );

    if !is_bottom_right_step {
        log("Current step doesn't expect bottom-right corner (F3).");
        return Ok(());
    }

    // Need top-left first
    let (tl_x, tl_y) = match ctx.pending_top_left {
        Some(pos) => pos,
        None => {
            log("Error: Record top-left corner (F2) first.");
            return Ok(());
        }
    };

    // Get cursor position and convert to relative
    let (screen_x, screen_y) = get_cursor_position()?;
    let game_hwnd = isize_to_hwnd(ctx.game_hwnd);
    let (br_x, br_y) = match screen_to_relative(game_hwnd, screen_x, screen_y) {
        Ok(pos) => pos,
        Err(e) => {
            log(&format!("Error: {}", e));
            return Ok(());
        }
    };

    // Calculate region
    let width = br_x - tl_x;
    let height = br_y - tl_y;

    if width <= 0.0 || height <= 0.0 {
        log("Error: Bottom-right must be below and to the right of top-left.");
        log("Press F2 to redo top-left, then F3 for bottom-right.");
        ctx.pending_top_left = None;
        return Ok(());
    }

    log(&format!(
        "Region: ({:.3}, {:.3}) size ({:.3} x {:.3})",
        tl_x, tl_y, width, height
    ));

    let region = RelativeRect {
        x: tl_x,
        y: tl_y,
        width,
        height,
    };

    // Store the region
    match ctx.current_step {
        CalibrationStep::SkipButtonRegionBottomRight => {
            ctx.items.skip_button_region = Some(region);
        }
        CalibrationStep::ScoreRegionBottomRight { stage, character } => {
            ctx.items.score_regions[stage][character] = Some(region);
        }
        CalibrationStep::StageTotalRegionBottomRight { stage } => {
            ctx.items.stage_total_regions[stage] = Some(region);
        }
        _ => {}
    }

    ctx.pending_top_left = None;

    // Show preview and ask for confirmation
    show_step_preview(ctx)?;
    ctx.awaiting_confirmation = true;
    log("Press Y to confirm, N to redo.");

    Ok(())
}

/// Shows a preview of the current step.
fn show_step_preview(ctx: &CalibrationContext) -> Result<()> {
    // Capture screenshot
    let game_hwnd = isize_to_hwnd(ctx.game_hwnd);
    let screenshot = capture_gakumas_to_buffer(game_hwnd)?;

    // Build partial config for preview
    let base_config = get_config();
    let mut preview_config = base_config.clone();

    // Update with captured values
    if let Some(ref btn) = ctx.items.start_button {
        preview_config.start_button = btn.clone();
    }
    if let Some(ref btn) = ctx.items.skip_button {
        preview_config.skip_button = btn.clone();
    }
    if let Some(ref region) = ctx.items.skip_button_region {
        preview_config.skip_button_region = region.clone();
    }

    // Build score_regions array from captured items
    let mut has_any_score = false;
    let mut score_regions = [[RelativeRect::default(); 3]; 3];
    for stage in 0..3 {
        for character in 0..3 {
            if let Some(ref region) = ctx.items.score_regions[stage][character] {
                score_regions[stage][character] = region.clone();
                has_any_score = true;
            }
        }
    }
    if has_any_score {
        preview_config.score_regions = Some(score_regions);
    }

    // Build stage_total_regions array from captured items
    let mut has_any_total = false;
    let mut total_regions = [RelativeRect::default(); 3];
    for stage in 0..3 {
        if let Some(ref region) = ctx.items.stage_total_regions[stage] {
            total_regions[stage] = region.clone();
            has_any_total = true;
        }
    }
    if has_any_total {
        preview_config.stage_total_regions = Some(total_regions);
    }

    // Determine highlight
    let highlight = match ctx.current_step {
        CalibrationStep::StartButton => Some(HighlightedItem::StartButton),
        CalibrationStep::SkipButton => Some(HighlightedItem::SkipButton),
        CalibrationStep::SkipButtonRegionBottomRight => Some(HighlightedItem::SkipButtonRegion),
        CalibrationStep::ScoreRegionBottomRight { stage, character } => {
            Some(HighlightedItem::ScoreRegion { stage, character })
        }
        CalibrationStep::StageTotalRegionBottomRight { stage } => {
            Some(HighlightedItem::StageTotalRegion { stage })
        }
        _ => None,
    };

    // Render and show preview
    let preview = if let Some(h) = highlight {
        render_preview_with_highlight(&screenshot, &preview_config, &h)
    } else {
        render_preview(&screenshot, &preview_config)
    };

    show_preview(&preview, "calibration_preview.png")?;
    log("Preview opened in default viewer.");

    Ok(())
}

/// Advances to the next calibration step.
fn advance_to_next_step(ctx: &mut CalibrationContext) -> Result<()> {
    log("Confirmed.");
    log("");

    ctx.current_step = match ctx.current_step {
        CalibrationStep::StartButton => CalibrationStep::SkipButton,
        CalibrationStep::SkipButton => CalibrationStep::SkipButtonRegionTopLeft,
        CalibrationStep::SkipButtonRegionBottomRight => {
            CalibrationStep::ScoreRegionTopLeft { stage: 0, character: 0 }
        }
        CalibrationStep::ScoreRegionBottomRight { stage, character } => {
            if character < 2 {
                CalibrationStep::ScoreRegionTopLeft { stage, character: character + 1 }
            } else if stage < 2 {
                CalibrationStep::ScoreRegionTopLeft { stage: stage + 1, character: 0 }
            } else {
                CalibrationStep::StageTotalRegionTopLeft { stage: 0 }
            }
        }
        CalibrationStep::StageTotalRegionBottomRight { stage } => {
            if stage < 2 {
                CalibrationStep::StageTotalRegionTopLeft { stage: stage + 1 }
            } else {
                CalibrationStep::Complete
            }
        }
        _ => ctx.current_step.clone(),
    };

    if !matches!(ctx.current_step, CalibrationStep::Complete) {
        print_step_instructions(&ctx.current_step);
    }

    Ok(())
}

/// Rewinds to the start of the current item (e.g., BottomRight -> TopLeft).
/// Used when user presses N to redo.
fn rewind_to_step_start(step: &CalibrationStep) -> CalibrationStep {
    match step {
        // Button steps stay the same
        CalibrationStep::StartButton => CalibrationStep::StartButton,
        CalibrationStep::SkipButton => CalibrationStep::SkipButton,
        // Region steps go back to TopLeft
        CalibrationStep::SkipButtonRegionTopLeft
        | CalibrationStep::SkipButtonRegionBottomRight => CalibrationStep::SkipButtonRegionTopLeft,
        CalibrationStep::ScoreRegionTopLeft { stage, character }
        | CalibrationStep::ScoreRegionBottomRight { stage, character } => {
            CalibrationStep::ScoreRegionTopLeft {
                stage: *stage,
                character: *character,
            }
        }
        CalibrationStep::StageTotalRegionTopLeft { stage }
        | CalibrationStep::StageTotalRegionBottomRight { stage } => {
            CalibrationStep::StageTotalRegionTopLeft { stage: *stage }
        }
        CalibrationStep::Complete => CalibrationStep::Complete,
    }
}

/// Skips the current step without recording (keeps existing config value).
fn skip_current_step(ctx: &mut CalibrationContext) -> Result<()> {
    // Skip to the next "start" step (not intermediate steps like BottomRight)
    ctx.current_step = match ctx.current_step {
        CalibrationStep::StartButton => CalibrationStep::SkipButton,
        CalibrationStep::SkipButton => CalibrationStep::SkipButtonRegionTopLeft,
        CalibrationStep::SkipButtonRegionTopLeft | CalibrationStep::SkipButtonRegionBottomRight => {
            CalibrationStep::ScoreRegionTopLeft { stage: 0, character: 0 }
        }
        CalibrationStep::ScoreRegionTopLeft { stage, character }
        | CalibrationStep::ScoreRegionBottomRight { stage, character } => {
            if character < 2 {
                CalibrationStep::ScoreRegionTopLeft { stage, character: character + 1 }
            } else if stage < 2 {
                CalibrationStep::ScoreRegionTopLeft { stage: stage + 1, character: 0 }
            } else {
                CalibrationStep::StageTotalRegionTopLeft { stage: 0 }
            }
        }
        CalibrationStep::StageTotalRegionTopLeft { stage }
        | CalibrationStep::StageTotalRegionBottomRight { stage } => {
            if stage < 2 {
                CalibrationStep::StageTotalRegionTopLeft { stage: stage + 1 }
            } else {
                CalibrationStep::Complete
            }
        }
        CalibrationStep::Complete => CalibrationStep::Complete,
    };

    ctx.pending_top_left = None;

    if !matches!(ctx.current_step, CalibrationStep::Complete) {
        print_step_instructions(&ctx.current_step);
    }

    Ok(())
}

/// Prints instructions for the current step.
fn print_current_step_instructions() {
    let guard = CALIBRATION.lock().unwrap();
    if let Some(ctx) = guard.as_ref() {
        print_step_instructions(&ctx.current_step);
    }
}

/// Prints instructions for a specific step.
fn print_step_instructions(step: &CalibrationStep) {
    let step_num = step.step_number();
    let total = CalibrationStep::total_steps();
    let desc = step.description();

    log(&format!("Step {}/{}: {}", step_num, total, desc));

    match step {
        CalibrationStep::StartButton => {
            log("Position cursor over the CENTER of the Start button (開始する).");
            log("Press F1 to record.");
        }
        CalibrationStep::SkipButton => {
            log("Position cursor over the CENTER of the Skip button (スキップ).");
            log("Press F1 to record.");
        }
        CalibrationStep::SkipButtonRegionTopLeft => {
            log("This region detects if the skip button is visible (brightness check).");
            log("Position cursor at TOP-LEFT corner of the skip button area.");
            log("Press F2 to record top-left.");
        }
        CalibrationStep::ScoreRegionTopLeft { stage, character } => {
            log(&format!(
                "Score region for Stage {} Character {} (S{}C{}).",
                stage + 1,
                character + 1,
                stage + 1,
                character + 1
            ));
            log("Position cursor at TOP-LEFT corner of the score number.");
            log("Press F2 to record top-left.");
        }
        CalibrationStep::StageTotalRegionTopLeft { stage } => {
            log(&format!("Stage {} Total score region.", stage + 1));
            log("Position cursor at TOP-LEFT corner of the total score.");
            log("Press F2 to record top-left.");
        }
        _ => {}
    }
}

/// Finishes calibration and saves config.
fn finish_calibration(items: CalibrationItems, game_hwnd: HWND) -> Result<()> {
    log("");
    log("=======================================================");
    log("           CALIBRATION COMPLETE!");
    log("=======================================================");
    log("");

    // Build final config
    let base_config = get_config();
    let mut final_config = base_config.clone();

    // Apply captured values
    if let Some(btn) = items.start_button {
        final_config.start_button = btn;
    }
    if let Some(btn) = items.skip_button {
        final_config.skip_button = btn;
    }
    if let Some(region) = items.skip_button_region {
        final_config.skip_button_region = region;
    }

    // Build score regions array
    let mut score_regions_complete = true;
    let mut score_regions = [[RelativeRect::default(); 3]; 3];
    for stage in 0..3 {
        for character in 0..3 {
            if let Some(region) = &items.score_regions[stage][character] {
                score_regions[stage][character] = region.clone();
            } else {
                score_regions_complete = false;
            }
        }
    }
    if score_regions_complete {
        final_config.score_regions = Some(score_regions);
    }

    // Build stage total regions array
    let mut total_regions_complete = true;
    let mut total_regions = [RelativeRect::default(); 3];
    for stage in 0..3 {
        if let Some(region) = &items.stage_total_regions[stage] {
            total_regions[stage] = region.clone();
        } else {
            total_regions_complete = false;
        }
    }
    if total_regions_complete {
        final_config.stage_total_regions = Some(total_regions);
    }

    // Save config
    let config_path = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("config.json")))
        .unwrap_or_else(|| std::path::Path::new("config.json").to_path_buf());

    let json = serde_json::to_string_pretty(&final_config)?;
    std::fs::write(&config_path, &json)?;
    log(&format!("Config saved to: {}", config_path.display()));

    // Show final preview
    log("Generating final preview...");
    if let Ok(screenshot) = capture_gakumas_to_buffer(game_hwnd) {
        let preview = render_preview(&screenshot, &final_config);
        if show_preview(&preview, "calibration_complete.png").is_ok() {
            log("Final preview opened.");
        }
    }

    log("");
    log("Calibration finished. You may need to restart the app to use new config.");

    stop_calibration()?;
    Ok(())
}

/// Stops calibration and unregisters hotkeys.
pub fn stop_calibration() -> Result<()> {
    let mut guard = CALIBRATION.lock().unwrap();
    if let Some(ctx) = guard.take() {
        let app_hwnd = isize_to_hwnd(ctx.app_hwnd);
        unregister_calibration_hotkeys(app_hwnd);
        log("Calibration mode ended.");
    }
    Ok(())
}

/// Registers calibration-specific hotkeys.
fn register_calibration_hotkeys(hwnd: HWND) -> Result<()> {
    unsafe {
        RegisterHotKey(hwnd, HOTKEY_CAL_F1, MOD_NOREPEAT, VK_F1.0 as u32)?;
        RegisterHotKey(hwnd, HOTKEY_CAL_F2, MOD_NOREPEAT, VK_F2.0 as u32)?;
        RegisterHotKey(hwnd, HOTKEY_CAL_F3, MOD_NOREPEAT, VK_F3.0 as u32)?;
        RegisterHotKey(hwnd, HOTKEY_CAL_Y, MOD_NOREPEAT, VK_Y.0 as u32)?;
        RegisterHotKey(hwnd, HOTKEY_CAL_N, MOD_NOREPEAT, VK_N.0 as u32)?;
        RegisterHotKey(hwnd, HOTKEY_CAL_ESCAPE, MOD_NOREPEAT, VK_ESCAPE.0 as u32)?;
        RegisterHotKey(hwnd, HOTKEY_CAL_ENTER, MOD_NOREPEAT, VK_RETURN.0 as u32)?;
    }
    Ok(())
}

/// Unregisters calibration-specific hotkeys.
fn unregister_calibration_hotkeys(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_F1);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_F2);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_F3);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_Y);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_N);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_ESCAPE);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CAL_ENTER);
    }
}

/// Shows a one-shot preview of all configured regions.
pub fn show_preview_once() -> Result<()> {
    let game_hwnd = find_gakumas_window()?;
    let config = get_config();

    log("Capturing screenshot for preview...");
    let screenshot = capture_gakumas_to_buffer(game_hwnd)?;
    let preview = render_preview(&screenshot, config);
    show_preview(&preview, "regions_preview.png")?;
    log("Preview opened: regions_preview.png");

    Ok(())
}
