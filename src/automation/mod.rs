//! UI automation functionality for the gakumas game.
//!
//! This module provides input simulation capabilities for automating
//! interactions with the game window.

pub mod config;
pub mod detection;
pub mod input;

pub use config::{get_config, init_config, AutomationConfig, ButtonConfig, RelativeRect};
pub use detection::{calculate_brightness, measure_region_brightness, wait_for_loading};
pub use input::{click_at_relative, test_postmessage_click, test_sendinput_click};
