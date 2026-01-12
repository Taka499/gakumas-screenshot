//! Screen capture functionality for the gakumas.exe game window.
//!
//! This module provides:
//! - Window discovery (`find_gakumas_window`)
//! - Client area information (`get_client_area_info`)
//! - Screenshot capture (`capture_gakumas`)

pub mod screenshot;
pub mod window;

pub use screenshot::capture_gakumas;
pub use window::find_gakumas_window;
// Re-exported for future use by automation and other modules
#[allow(unused_imports)]
pub use window::get_client_area_info;
