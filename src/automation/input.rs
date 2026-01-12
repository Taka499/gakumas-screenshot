//! Mouse input simulation for UI automation.
//!
//! This module provides functions for simulating mouse clicks on the game window.
//! Two methods are implemented:
//! - PostMessage: Sends window messages directly (does not work with the game)
//! - SendInput: Simulates hardware-level input (works, but moves the actual cursor)

use anyhow::{anyhow, Result};

use windows::Win32::Foundation::{LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetSystemMetrics, PostMessageW, SetForegroundWindow, SM_CXSCREEN, SM_CYSCREEN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};

use crate::capture::find_gakumas_window;

/// Tests if PostMessage-based clicking works with the game.
///
/// This sends WM_LBUTTONDOWN/UP messages to the center of the game's client area.
/// Note: This method does NOT work with the game because it validates focus state.
/// The function is kept for reference and comparison testing.
pub fn test_postmessage_click() -> Result<()> {
    crate::log("Testing PostMessage click...");

    let hwnd = find_gakumas_window()?;
    crate::log(&format!("Found window: {:?}", hwnd));

    // Get client area size
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client_rect)? };

    let client_width = client_rect.right - client_rect.left;
    let client_height = client_rect.bottom - client_rect.top;

    // Click at center of client area
    let click_x = client_width / 2;
    let click_y = client_height / 2;

    crate::log(&format!(
        "Client area: {}x{}, clicking at ({}, {})",
        client_width, client_height, click_x, click_y
    ));

    // Pack coordinates into LPARAM: low word = x, high word = y
    let lparam = LPARAM(((click_y as u32) << 16 | (click_x as u32)) as isize);

    // WPARAM for mouse buttons: MK_LBUTTON = 0x0001
    let wparam_down = WPARAM(0x0001); // MK_LBUTTON
    let wparam_up = WPARAM(0);

    unsafe {
        // First, send mouse move to position (some apps need this)
        crate::log("Sending WM_MOUSEMOVE...");
        let move_result = PostMessageW(hwnd, WM_MOUSEMOVE, WPARAM(0), lparam);
        crate::log(&format!("WM_MOUSEMOVE result: {:?}", move_result));

        // Small delay
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Send mouse down
        crate::log("Sending WM_LBUTTONDOWN...");
        let down_result = PostMessageW(hwnd, WM_LBUTTONDOWN, wparam_down, lparam);
        crate::log(&format!("WM_LBUTTONDOWN result: {:?}", down_result));

        // Small delay between down and up
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Send mouse up
        crate::log("Sending WM_LBUTTONUP...");
        let up_result = PostMessageW(hwnd, WM_LBUTTONUP, wparam_up, lparam);
        crate::log(&format!("WM_LBUTTONUP result: {:?}", up_result));
    }

    crate::log("PostMessage click sequence completed");
    Ok(())
}

/// Tests if SendInput-based clicking works with the game.
///
/// WARNING: This WILL move your actual cursor to the game window center.
///
/// This method works reliably with the game because it simulates hardware-level
/// input that the game's input layer (DirectInput/RawInput) processes correctly.
/// The window must be brought to foreground before sending input.
pub fn test_sendinput_click() -> Result<()> {
    crate::log("Testing SendInput click...");

    let hwnd = find_gakumas_window()?;
    crate::log(&format!("Found window: {:?}", hwnd));

    // Bring window to foreground
    crate::log("Bringing window to foreground...");
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    // Give window time to activate
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Get client area size
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client_rect)? };

    let client_width = client_rect.right - client_rect.left;
    let client_height = client_rect.bottom - client_rect.top;

    // Click at center of client area (in client coordinates)
    let click_x = client_width / 2;
    let click_y = client_height / 2;

    // Convert client coordinates to screen coordinates
    let mut screen_point = POINT {
        x: click_x,
        y: click_y,
    };
    unsafe {
        if !ClientToScreen(hwnd, &mut screen_point).as_bool() {
            return Err(anyhow!("ClientToScreen failed"));
        }
    }

    crate::log(&format!(
        "Client area: {}x{}, clicking at client ({}, {}) = screen ({}, {})",
        client_width, client_height, click_x, click_y, screen_point.x, screen_point.y
    ));

    // Get screen dimensions for normalization
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    // Normalize to 0-65535 range (required by MOUSEEVENTF_ABSOLUTE)
    let norm_x = ((screen_point.x as i64 * 65535) / screen_width as i64) as i32;
    let norm_y = ((screen_point.y as i64 * 65535) / screen_height as i64) as i32;

    crate::log(&format!(
        "Screen: {}x{}, normalized coords: ({}, {})",
        screen_width, screen_height, norm_x, norm_y
    ));

    unsafe {
        // Move + click in one sequence with absolute coordinates on each event
        crate::log("Sending mouse move...");
        let move_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: norm_x,
                    dy: norm_y,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    ..Default::default()
                },
            },
        };
        let move_result = SendInput(&[move_input], std::mem::size_of::<INPUT>() as i32);
        crate::log(&format!("Mouse move result: {} inputs sent", move_result));

        std::thread::sleep(std::time::Duration::from_millis(100));

        // Mouse down with absolute position
        crate::log("Sending mouse down at absolute position...");
        let down_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: norm_x,
                    dy: norm_y,
                    dwFlags: MOUSEEVENTF_LEFTDOWN | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE,
                    ..Default::default()
                },
            },
        };
        let down_result = SendInput(&[down_input], std::mem::size_of::<INPUT>() as i32);
        crate::log(&format!("Mouse down result: {} inputs sent", down_result));

        std::thread::sleep(std::time::Duration::from_millis(50));

        // Mouse up with absolute position
        crate::log("Sending mouse up at absolute position...");
        let up_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: norm_x,
                    dy: norm_y,
                    dwFlags: MOUSEEVENTF_LEFTUP | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE,
                    ..Default::default()
                },
            },
        };
        let up_result = SendInput(&[up_input], std::mem::size_of::<INPUT>() as i32);
        crate::log(&format!("Mouse up result: {} inputs sent", up_result));
    }

    crate::log("SendInput click sequence completed");
    Ok(())
}
