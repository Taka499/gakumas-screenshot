//! UI automation functionality for the gakumas game.
//!
//! This module provides input simulation capabilities for automating
//! interactions with the game window.

pub mod input;

pub use input::{test_postmessage_click, test_sendinput_click};
