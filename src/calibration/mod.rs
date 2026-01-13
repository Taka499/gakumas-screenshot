//! Calibration module for interactive region and button position configuration.
//!
//! Provides a wizard that guides users through defining screen regions
//! by pointing and pressing hotkeys, with visual preview after each step.

pub mod coords;
pub mod preview;
pub mod state;
pub mod wizard;

pub use wizard::{
    handle_calibration_hotkey, is_calibrating, show_preview_once, start_calibration,
    HOTKEY_CAL_ENTER, HOTKEY_CAL_ESCAPE, HOTKEY_CAL_F1, HOTKEY_CAL_F2, HOTKEY_CAL_F3,
    HOTKEY_CAL_N, HOTKEY_CAL_Y,
};
