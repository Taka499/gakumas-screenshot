//! UI automation functionality for the gakumas game.
//!
//! This module provides:
//! - Input simulation for automating game interactions
//! - Loading state detection via brightness analysis
//! - Asynchronous automation loop with OCR processing
//! - CSV result output

pub mod config;
pub mod csv_writer;
pub mod detection;
pub mod input;
pub mod ocr_worker;
pub mod queue;
pub mod runner;
pub mod state;

pub use config::{get_config, init_config, AutomationConfig, ButtonConfig, RelativeRect};
pub use detection::{
    calculate_brightness, load_reference_histogram, measure_region_brightness,
    save_end_button_reference, save_skip_button_reference, save_start_button_reference,
    wait_for_loading, wait_for_result, wait_for_start_page,
};
pub use input::{click_at_relative, test_postmessage_click, test_sendinput_click};
pub use runner::{is_automation_running, request_abort, start_automation};
