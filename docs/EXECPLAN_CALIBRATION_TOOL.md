# Calibration Tool for Score Region and Button Position Configuration

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds. This document must be maintained in accordance with `docs/PLANS.md`.


## Purpose / Big Picture

After this change, users can interactively define the screen regions where scores appear and where buttons are located, without manually editing JSON coordinates. The user runs the application with `--calibrate`, positions their mouse cursor over the game window, and presses hotkeys to record each location. The tool converts screen positions to relative coordinates (0.0-1.0) that work regardless of window size, then saves everything to `config.json`.

A key feature is visual preview: during calibration, after each region is defined, the tool captures a screenshot with the newly defined region highlighted so users can immediately verify accuracy. Additionally, a `--preview` mode provides continuous live preview that refreshes automatically, allowing users to see all configured regions overlaid on the current game screen.

This is a prerequisite for Phase 2 (OCR) and Phase 3 (Automation), which need accurate region definitions to function.


## Progress

- [x] Milestone 1: ~~CLI argument parsing with clap~~ Tray menu integration
- [x] Milestone 2: Calibration mode infrastructure
- [x] Milestone 3: Preview rendering system
- [x] Milestone 4: Interactive point/region capture with per-step preview
- [x] Milestone 5: ~~Live preview mode (--preview)~~ "Preview Regions" menu item (one-shot)
- [x] Milestone 6: Config serialization and verification
- [x] Milestone 7: End-to-end validation (tested manually)


## Surprises & Discoveries

- The application requires administrator privileges (from manifest), so CLI arguments don't work well with elevated processes. Switched to tray menu integration instead.


## Decision Log

- Decision: Use tray menu instead of CLI arguments for calibration entry
  Rationale: The application requires administrator privileges (manifest), making CLI arguments problematic with UAC and terminal elevation. Tray menu integrates naturally with the existing elevated process and provides familiar UX.
  Date/Author: 2026-01-14 / Implementation change

- Decision: Use hotkey-based calibration rather than GUI overlay
  Rationale: Building a transparent overlay window with click handling is complex. Hotkeys are simpler and the application already has hotkey infrastructure. User can see the game window directly without overlay interference.
  Date/Author: 2026-01-13 / Initial design

- Decision: Capture both individual scores (9) and stage totals (3) for validation
  Rationale: Stage totals can verify OCR accuracy by checking sum of individual scores against total.
  Date/Author: 2026-01-13 / Initial design

- Decision: Use clap for CLI argument parsing
  Rationale: Industry standard, derives work well with Rust structs, already mentioned in roadmap.
  Date/Author: 2026-01-13 / Initial design

- Decision: Add visual preview system with three modes
  Rationale: Users cannot mentally map relative coordinates to screen positions. Visual feedback is essential for accurate calibration. Three modes serve different needs: (1) per-step preview during calibration for immediate verification, (2) live preview mode for iterative adjustment, (3) one-shot verify for final confirmation.
  Date/Author: 2026-01-13 / Enhancement

- Decision: Save preview images to files and open with system default viewer
  Rationale: Building a custom preview window adds complexity (Win32 GDI, message loop). Using the system image viewer is simpler and users are familiar with it. Preview images are saved as PNG files which can also be shared for debugging.
  Date/Author: 2026-01-13 / Enhancement


## Outcomes & Retrospective

**Completed: 2026-01-14**

### What was delivered
- Tray menu integration with "Calibrate Regions..." and "Preview Regions" options
- Interactive calibration wizard with 15 steps (2 buttons + 1 brightness region + 9 score regions + 3 stage totals)
- Hotkey-based capture: F1 (point), F2 (top-left), F3 (bottom-right), Y (confirm), N (redo), Enter (skip), Escape (abort)
- Visual preview after each step showing captured regions highlighted
- Config serialization with new `score_regions` and `stage_total_regions` fields
- Skip functionality to keep existing values from config.json

### What worked well
- Tray menu approach was simpler than CLI args given admin privilege requirements
- Hotkey-based capture worked smoothly without needing complex overlay UI
- Preview images in system viewer provided immediate visual feedback

### Lessons learned
- Windows apps requiring elevation don't work well with CLI arguments from terminals
- Region capture needs both TopLeft and BottomRight steps; redo (N) must rewind to TopLeft
- Global Mutex with HWND requires storing as isize for Send+Sync safety


## Context and Orientation

The gakumas-screenshot application is a Windows system tray tool that captures screenshots of the game "Gakuen iDOLM@STER". It currently supports:

- Finding the game window by process name (`gakumas.exe`)
- Capturing the window via Windows Graphics Capture API
- Sending mouse clicks to relative coordinates
- Detecting loading state via brightness analysis

Key files:

    src/main.rs                    - Entry point, message loop, hotkey handling
    src/capture/window.rs          - Window discovery (find_gakumas_window)
    src/capture/screenshot.rs      - Full window capture
    src/capture/region.rs          - Partial region capture
    src/automation/config.rs       - Configuration types and loading
    src/automation/input.rs        - Mouse input simulation
    config.json                    - Runtime configuration file

The current `config.json` structure:

    {
      "start_button": { "x": 0.5, "y": 0.85 },
      "skip_button": { "x": 0.82, "y": 0.82 },
      "skip_button_region": { "x": 0.7, "y": 0.80, "width": 0.22, "height": 0.04 },
      "brightness_threshold": 150.0,
      "loading_timeout_ms": 30000,
      "capture_delay_ms": 500,
      "test_click_position": { "x": 0.92, "y": 0.84 }
    }

Terms used in this document:

- Relative coordinates: Values from 0.0 to 1.0 representing position as fraction of window size. (0.0, 0.0) is top-left, (1.0, 1.0) is bottom-right.
- Client area: The drawable portion of a window, excluding title bar and borders.
- HWND: Windows handle to a window object.
- Score region: A rectangular area containing a single score number to be OCR'd.
- Stage total: The sum score for one stage (e.g., "195,601pt").


## Plan of Work

### Milestone 1: CLI Argument Parsing

Add the `clap` crate and create a command-line interface. The application will support these modes:

- Default (no args): Run as system tray application (current behavior)
- `--calibrate`: Enter calibration mode with per-step visual preview
- `--preview`: Live preview mode showing all configured regions (auto-refreshes)
- `--verify-calibration`: One-shot preview saved to file
- `--screenshot-only`: Take one screenshot and exit (future use)

In `Cargo.toml`, add:

    clap = { version = "4.0", features = ["derive"] }

In `src/main.rs`, add a `Cli` struct with clap derive macros before the `main()` function. Parse arguments at the start of `main()` and branch based on mode.


### Milestone 2: Calibration Mode Infrastructure

Create a new module `src/calibration/mod.rs` with the calibration logic. This module will:

1. Find the game window
2. Register calibration-specific hotkeys
3. Run a calibration message loop
4. Track which items have been calibrated

Create `src/calibration/state.rs` to hold calibration state:

    pub struct CalibrationState {
        pub hwnd: HWND,
        pub client_width: u32,
        pub client_height: u32,
        pub items: CalibrationItems,
        pub current_step: CalibrationStep,
    }

    pub struct CalibrationItems {
        pub start_button: Option<ButtonConfig>,
        pub skip_button: Option<ButtonConfig>,
        pub skip_button_region: Option<RelativeRect>,
        pub score_regions: [[Option<RelativeRect>; 3]; 3],
        pub stage_total_regions: [Option<RelativeRect>; 3],
    }

    pub enum CalibrationStep {
        StartButton,
        SkipButton,
        SkipButtonRegion,
        ScoreRegion { stage: usize, character: usize },
        StageTotalRegion { stage: usize },
        Complete,
    }


### Milestone 3: Preview Rendering System

Create `src/calibration/preview.rs` to handle all preview rendering. This module draws regions and points on a screenshot image.

Drawing specifications:
- Score regions (9): Green rectangles with 2px border, labeled "S1C1", "S1C2", etc.
- Stage total regions (3): Blue rectangles with 2px border, labeled "S1 Total", etc.
- Button positions (2): Red crosshairs (10px arms) with labels "Start", "Skip"
- Skip button brightness region: Yellow rectangle with 2px border, labeled "Brightness"
- Pending region (during calibration): Orange dashed rectangle showing current selection

The preview module provides:

    /// Renders all configured regions onto a screenshot.
    pub fn render_preview(
        screenshot: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        config: &AutomationConfig,
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>>;

    /// Renders preview with a single highlighted region (for per-step preview).
    pub fn render_preview_with_highlight(
        screenshot: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        config: &AutomationConfig,
        highlight: &HighlightedItem,
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>>;

    /// Saves preview image and opens with system default viewer.
    pub fn show_preview(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, filename: &str) -> Result<()>;

    pub enum HighlightedItem {
        Button(ButtonType),
        Region(RegionType),
    }

The `show_preview` function:
1. Saves image to a temporary file (e.g., `calibration_step_preview.png`)
2. Opens with Windows shell execute (`ShellExecuteW` with "open" verb)
3. User views in their default image viewer


### Milestone 4: Interactive Point/Region Capture with Per-Step Preview

Implement the calibration interaction flow:

1. Display instructions in console for current step
2. User positions cursor over game window
3. For points (buttons): User presses F1 to record cursor position
4. For regions: User presses F2 at top-left corner, then F3 at bottom-right corner
5. Convert screen coordinates to relative coordinates
6. **Show preview**: Capture screenshot, render with newly defined item highlighted, open in viewer
7. User confirms in console (Y to accept, N to redo current step)
8. Advance to next step
9. Press Escape to abort, Enter to skip optional items

The per-step preview flow:

    // After recording a point or completing a region
    let screenshot = capture_gakumas()?;
    let preview = render_preview_with_highlight(&screenshot, &partial_config, &current_item);
    show_preview(&preview, "calibration_step_preview.png")?;

    println!("Preview opened. Is the position correct? [Y/n]");
    if !user_confirms() {
        // Redo current step
        continue;
    }

The coordinate conversion logic (in `src/calibration/coords.rs`):

    pub fn screen_to_relative(
        hwnd: HWND,
        screen_x: i32,
        screen_y: i32,
    ) -> Result<(f32, f32)> {
        // 1. Get client area origin in screen coordinates
        // 2. Subtract to get client-relative position
        // 3. Divide by client dimensions to get 0.0-1.0 range
    }


### Milestone 5: Live Preview Mode (--preview)

Implement a continuous preview mode that:

1. Loads current `config.json`
2. Captures game screenshot
3. Renders all configured regions on the screenshot
4. Saves and opens preview image
5. Waits 2 seconds, then repeats (or until user presses Ctrl+C)

This mode allows users to:
- Edit `config.json` manually and see changes immediately
- Fine-tune region positions iteratively
- Verify regions still work after game window resize

Implementation in `src/calibration/wizard.rs`:

    pub fn run_live_preview() -> Result<()> {
        let config = get_config();
        let hwnd = find_gakumas_window()?;

        println!("Live preview mode. Press Ctrl+C to exit.");
        println!("Edit config.json and save - preview will update automatically.");

        loop {
            // Check if config.json was modified
            let config = reload_config_if_changed()?;

            // Capture and render
            let screenshot = capture_gakumas_to_buffer(hwnd)?;
            let preview = render_preview(&screenshot, &config);
            show_preview(&preview, "live_preview.png")?;

            // Wait before next refresh
            std::thread::sleep(Duration::from_secs(2));
        }
    }

The preview image filename includes a counter or timestamp to ensure the image viewer refreshes:

    live_preview_001.png, live_preview_002.png, ...

Old preview files are cleaned up after a few iterations to avoid cluttering the directory.


### Milestone 6: Config Serialization and Verification

After all items are captured:

1. Build the complete `AutomationConfig` struct
2. Serialize to JSON with `serde_json::to_string_pretty`
3. Write to `config.json` next to executable
4. Capture a verification screenshot with regions overlaid (optional but helpful)

Extend `AutomationConfig` in `src/automation/config.rs` to include the new fields:

    pub struct AutomationConfig {
        // Existing fields...
        pub start_button: ButtonConfig,
        pub skip_button: ButtonConfig,
        pub skip_button_region: RelativeRect,
        pub brightness_threshold: f32,
        pub loading_timeout_ms: u64,
        pub capture_delay_ms: u64,

        // New fields for OCR
        pub score_regions: [[RelativeRect; 3]; 3],      // [stage][character]
        pub stage_total_regions: [RelativeRect; 3],     // [stage]
    }


### Milestone 7: End-to-End Validation

The `--verify-calibration` flag provides a one-shot preview:

1. Loads `config.json`
2. Captures the current game screen
3. Draws rectangles on the captured image showing all defined regions
4. Saves as `calibration_preview.png`
5. Opens the image in default viewer
6. User visually confirms regions are correct

This is useful for:
- Quick verification after manual config edits
- Sharing calibration screenshots for debugging
- Checking calibration before running automation


## Concrete Steps

All commands run from repository root: `C:\Work\GitRepos\gakumas-screenshot`


### Step 1: Add clap dependency

Edit `Cargo.toml` to add clap. Then verify it compiles:

    cargo build --release

Expected: Build succeeds with clap downloaded.


### Step 2: Create calibration module structure

Create these new files:

    src/calibration/mod.rs
    src/calibration/state.rs
    src/calibration/coords.rs
    src/calibration/preview.rs
    src/calibration/wizard.rs

Add `mod calibration;` to `src/main.rs`.

Verify:

    cargo build --release

Expected: Build succeeds.


### Step 3: Implement CLI parsing

Modify `src/main.rs` to parse arguments and branch to calibration mode.

Verify:

    .\target\release\gakumas-screenshot.exe --help

Expected output includes:

    Usage: gakumas-screenshot.exe [OPTIONS]

    Options:
          --calibrate           Run calibration wizard with visual preview
          --preview             Live preview mode (refreshes every 2 seconds)
          --verify-calibration  One-shot preview saved to calibration_preview.png
      -h, --help                Print help


### Step 4: Implement preview rendering

Implement `src/calibration/preview.rs` with drawing functions.

Verify by creating a simple test that loads a sample image and draws test regions:

    cargo test calibration::preview::tests::test_render_preview -- --nocapture

Expected: Test passes, creates a test preview image with colored rectangles.


### Step 5: Implement calibration wizard with per-step preview

Implement the interactive wizard in `src/calibration/wizard.rs`.

Verify by running:

    .\target\release\gakumas-screenshot.exe --calibrate

Expected: Console shows instructions like:

    === Gakumas Calibration Wizard ===

    Step 1/15: Start Button (開始する)
    Position your cursor over the CENTER of the Start button.
    Press F1 to record position, Escape to abort.

    Current cursor position: (0.52, 0.83)

After pressing F1:

    Position recorded: (0.52, 0.83)
    Opening preview...
    Is the position correct? [Y/n]

A preview image opens showing the screenshot with a red crosshair at the recorded position.


### Step 6: Implement live preview mode

Implement `--preview` mode in `src/calibration/wizard.rs`.

Verify:

    .\target\release\gakumas-screenshot.exe --preview

Expected: Console shows:

    Live preview mode. Press Ctrl+C to exit.
    Edit config.json and save - preview will update automatically.

    [14:30:00] Preview updated: live_preview_001.png
    [14:30:02] Preview updated: live_preview_002.png
    ...

Preview images open in the default viewer and refresh every 2 seconds.


### Step 7: Implement one-shot verification mode

Implement `--verify-calibration` that draws all regions on a screenshot.

Verify:

    .\target\release\gakumas-screenshot.exe --verify-calibration

Expected: Creates and opens `calibration_preview.png` showing the game screenshot with colored rectangles overlaid on each defined region:
- Green: Score regions
- Blue: Stage total regions
- Red crosshairs: Button positions
- Yellow: Brightness detection region


## Validation and Acceptance

The calibration tool is complete when:

1. Running `--calibrate` walks through all 15 steps (2 buttons + 1 brightness region + 9 score regions + 3 stage totals) with visual preview after each step

2. After completing calibration, `config.json` contains all new fields with reasonable values (all between 0.0 and 1.0)

3. Running `--preview` shows live preview that:
   - Updates every 2 seconds automatically
   - Reflects changes to config.json without restart
   - Shows all configured regions with correct colors

4. Running `--verify-calibration` produces an image where:
   - Green rectangles mark score regions (labeled S1C1, S1C2, etc.)
   - Blue rectangles mark stage total regions (labeled S1 Total, etc.)
   - Red crosshairs mark button positions (labeled Start, Skip)
   - Yellow rectangle marks skip button brightness region

5. The existing screenshot hotkey (Ctrl+Shift+S) still works in normal mode

6. All regions from the sample image (`sample_rehearsal_result_page.png`) can be correctly captured and previewed


## Idempotence and Recovery

- Calibration can be rerun at any time; it overwrites `config.json`
- If calibration is aborted (Escape), existing `config.json` is not modified
- If the game window is not found, calibration exits with a clear error message
- Each recorded position is confirmed in the console before advancing


## Artifacts and Notes

Sample expected `config.json` after calibration:

    {
      "start_button": { "x": 0.50, "y": 0.85 },
      "skip_button": { "x": 0.82, "y": 0.82 },
      "skip_button_region": { "x": 0.70, "y": 0.80, "width": 0.22, "height": 0.04 },
      "brightness_threshold": 150.0,
      "loading_timeout_ms": 30000,
      "capture_delay_ms": 500,
      "score_regions": [
        [
          { "x": 0.12, "y": 0.18, "width": 0.08, "height": 0.03 },
          { "x": 0.42, "y": 0.18, "width": 0.08, "height": 0.03 },
          { "x": 0.72, "y": 0.18, "width": 0.08, "height": 0.03 }
        ],
        [
          { "x": 0.12, "y": 0.38, "width": 0.08, "height": 0.03 },
          { "x": 0.42, "y": 0.38, "width": 0.08, "height": 0.03 },
          { "x": 0.72, "y": 0.38, "width": 0.08, "height": 0.03 }
        ],
        [
          { "x": 0.12, "y": 0.58, "width": 0.08, "height": 0.03 },
          { "x": 0.42, "y": 0.58, "width": 0.08, "height": 0.03 },
          { "x": 0.72, "y": 0.58, "width": 0.08, "height": 0.03 }
        ]
      ],
      "stage_total_regions": [
        { "x": 0.35, "y": 0.12, "width": 0.15, "height": 0.04 },
        { "x": 0.35, "y": 0.32, "width": 0.15, "height": 0.04 },
        { "x": 0.35, "y": 0.52, "width": 0.15, "height": 0.04 }
      ]
    }


## Interfaces and Dependencies

### New Dependency

In `Cargo.toml`:

    clap = { version = "4.0", features = ["derive"] }


### New Module: src/calibration/mod.rs

    pub mod state;
    pub mod coords;
    pub mod preview;
    pub mod wizard;

    pub use state::{CalibrationState, CalibrationStep, CalibrationItems};
    pub use preview::{render_preview, render_preview_with_highlight, show_preview};
    pub use wizard::{run_calibration_wizard, run_live_preview, verify_calibration};


### New Types: src/calibration/state.rs

    use crate::automation::{ButtonConfig, RelativeRect};
    use windows::Win32::Foundation::HWND;

    pub struct CalibrationState {
        pub hwnd: HWND,
        pub client_width: u32,
        pub client_height: u32,
        pub items: CalibrationItems,
        pub current_step: CalibrationStep,
    }

    #[derive(Default)]
    pub struct CalibrationItems {
        pub start_button: Option<ButtonConfig>,
        pub skip_button: Option<ButtonConfig>,
        pub skip_button_region: Option<RelativeRect>,
        pub score_regions: [[Option<RelativeRect>; 3]; 3],
        pub stage_total_regions: [Option<RelativeRect>; 3],
    }

    pub enum CalibrationStep {
        StartButton,
        SkipButton,
        SkipButtonRegionTopLeft,
        SkipButtonRegionBottomRight,
        ScoreRegionTopLeft { stage: usize, character: usize },
        ScoreRegionBottomRight { stage: usize, character: usize },
        StageTotalRegionTopLeft { stage: usize },
        StageTotalRegionBottomRight { stage: usize },
        Complete,
    }


### New Function: src/calibration/coords.rs

    use anyhow::Result;
    use windows::Win32::Foundation::HWND;

    /// Converts screen coordinates to relative coordinates (0.0-1.0) within the window's client area.
    pub fn screen_to_relative(hwnd: HWND, screen_x: i32, screen_y: i32) -> Result<(f32, f32)>;

    /// Gets the current cursor position in screen coordinates.
    pub fn get_cursor_position() -> Result<(i32, i32)>;


### New Module: src/calibration/preview.rs

    use anyhow::Result;
    use image::{ImageBuffer, Rgba};
    use crate::automation::AutomationConfig;

    /// Color constants for preview rendering
    pub const COLOR_SCORE_REGION: Rgba<u8> = Rgba([0, 255, 0, 255]);      // Green
    pub const COLOR_TOTAL_REGION: Rgba<u8> = Rgba([0, 0, 255, 255]);      // Blue
    pub const COLOR_BUTTON: Rgba<u8> = Rgba([255, 0, 0, 255]);            // Red
    pub const COLOR_BRIGHTNESS: Rgba<u8> = Rgba([255, 255, 0, 255]);      // Yellow
    pub const COLOR_HIGHLIGHT: Rgba<u8> = Rgba([255, 128, 0, 255]);       // Orange

    /// What item to highlight in the preview
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
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>>;

    /// Renders preview with a single highlighted region (for per-step preview).
    pub fn render_preview_with_highlight(
        screenshot: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        config: &AutomationConfig,
        highlight: &HighlightedItem,
    ) -> ImageBuffer<Rgba<u8>, Vec<u8>>;

    /// Saves preview image and opens with system default viewer.
    pub fn show_preview(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, filename: &str) -> Result<()>;

    /// Draws a rectangle border on an image.
    fn draw_rect(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>, thickness: u32);

    /// Draws a crosshair at a point.
    fn draw_crosshair(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, color: Rgba<u8>, arm_length: u32);

    /// Draws a text label (simple bitmap font or skipped for MVP).
    fn draw_label(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, x: u32, y: u32, text: &str, color: Rgba<u8>);


### New Function: src/calibration/wizard.rs

    use anyhow::Result;

    /// Runs the interactive calibration wizard with per-step visual preview.
    /// Returns Ok(()) if calibration completed successfully and config was saved.
    /// Returns Err if aborted or failed.
    pub fn run_calibration_wizard() -> Result<()>;

    /// Runs live preview mode that refreshes every 2 seconds.
    /// Exits on Ctrl+C.
    pub fn run_live_preview() -> Result<()>;

    /// One-shot verification: captures screenshot with regions and opens it.
    pub fn verify_calibration() -> Result<()>;


### Extended Type: src/automation/config.rs

Add these fields to `AutomationConfig`:

    /// Score regions for OCR: [stage][character], 3 stages × 3 characters = 9 regions
    #[serde(default)]
    pub score_regions: [[RelativeRect; 3]; 3],

    /// Stage total regions for OCR validation: one per stage
    #[serde(default)]
    pub stage_total_regions: [RelativeRect; 3],


### CLI Entry Point: src/main.rs

    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(name = "gakumas-screenshot")]
    #[command(about = "Gakumas rehearsal screenshot and automation tool")]
    struct Cli {
        /// Run calibration wizard with per-step visual preview
        #[arg(long)]
        calibrate: bool,

        /// Live preview mode showing configured regions (refreshes every 2 seconds)
        #[arg(long)]
        preview: bool,

        /// One-shot verification: save preview to calibration_preview.png
        #[arg(long)]
        verify_calibration: bool,
    }

    // In main():
    fn main() -> Result<()> {
        let cli = Cli::parse();

        if cli.calibrate {
            return calibration::run_calibration_wizard();
        }
        if cli.preview {
            return calibration::run_live_preview();
        }
        if cli.verify_calibration {
            return calibration::verify_calibration();
        }

        // Default: run as system tray application
        run_tray_app()
    }


---

## Revision History

- 2026-01-13: Initial ExecPlan created
- 2026-01-13: Added visual preview system (Milestones 3, 5) with three modes:
  - Per-step preview during calibration (confirm each item visually)
  - Live preview mode (--preview) with auto-refresh
  - One-shot verification (--verify-calibration)
  Rationale: Users cannot mentally map relative coordinates to screen positions. Visual feedback is essential for accurate calibration.
