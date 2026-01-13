//! Coordinate conversion utilities.
//!
//! Converts screen coordinates to relative coordinates (0.0-1.0) within
//! the game window's client area.

use anyhow::{anyhow, Result};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetCursorPos};

/// Gets the current cursor position in screen coordinates.
pub fn get_cursor_position() -> Result<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe {
        GetCursorPos(&mut pt)?;
    }
    Ok((pt.x, pt.y))
}

/// Gets the client area dimensions of a window.
pub fn get_client_size(hwnd: HWND) -> Result<(u32, u32)> {
    let mut rect = RECT::default();
    unsafe {
        GetClientRect(hwnd, &mut rect)?;
    }
    Ok(((rect.right - rect.left) as u32, (rect.bottom - rect.top) as u32))
}

/// Gets the client area origin in screen coordinates.
pub fn get_client_origin(hwnd: HWND) -> Result<(i32, i32)> {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        if !ClientToScreen(hwnd, &mut pt).as_bool() {
            return Err(anyhow!("Failed to get client origin"));
        }
    }
    Ok((pt.x, pt.y))
}

/// Converts screen coordinates to relative coordinates (0.0-1.0) within the window's client area.
///
/// Returns an error if the cursor is outside the client area.
pub fn screen_to_relative(hwnd: HWND, screen_x: i32, screen_y: i32) -> Result<(f32, f32)> {
    let (origin_x, origin_y) = get_client_origin(hwnd)?;
    let (width, height) = get_client_size(hwnd)?;

    // Convert to client-relative coordinates
    let client_x = screen_x - origin_x;
    let client_y = screen_y - origin_y;

    // Check bounds
    if client_x < 0 || client_y < 0 || client_x >= width as i32 || client_y >= height as i32 {
        return Err(anyhow!(
            "Cursor is outside the game window's client area. \
             Position your cursor inside the game window."
        ));
    }

    // Convert to relative coordinates
    let rel_x = client_x as f32 / width as f32;
    let rel_y = client_y as f32 / height as f32;

    Ok((rel_x, rel_y))
}

/// Converts screen coordinates to relative coordinates, allowing positions outside the window.
/// Values outside 0.0-1.0 range indicate cursor is outside the client area.
pub fn screen_to_relative_unclamped(hwnd: HWND, screen_x: i32, screen_y: i32) -> Result<(f32, f32)> {
    let (origin_x, origin_y) = get_client_origin(hwnd)?;
    let (width, height) = get_client_size(hwnd)?;

    let client_x = screen_x - origin_x;
    let client_y = screen_y - origin_y;

    let rel_x = client_x as f32 / width as f32;
    let rel_y = client_y as f32 / height as f32;

    Ok((rel_x, rel_y))
}
