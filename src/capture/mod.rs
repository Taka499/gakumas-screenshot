//! Screen capture functionality for the gakumas.exe game window.
//!
//! This module provides:
//! - Window discovery (`find_gakumas_window`)
//! - Client area information (`get_client_area_info`)
//! - Screenshot capture (`capture_gakumas`)
//! - Region capture (`capture_region`)

pub mod region;
pub mod screenshot;
pub mod window;

pub use region::capture_region;
pub use screenshot::{capture_gakumas, capture_gakumas_to_buffer, capture_gakumas_to_buffer as capture_window_to_image};
pub use window::find_gakumas_window;
pub use window::get_client_area_info;
