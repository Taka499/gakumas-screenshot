//! Gakumas Screenshot Tool
//!
//! A Windows system tray application that captures screenshots of the
//! gakumas.exe game window using the Windows Graphics Capture API.

mod automation;
mod capture;

use anyhow::{anyhow, Result};
use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    GetCursorPos, GetMessageW, InsertMenuW, LoadIconW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, TrackPopupMenu, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
    IDI_APPLICATION, MF_BYPOSITION, MF_STRING, MSG, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TPM_RIGHTBUTTON, WM_COMMAND, WM_DESTROY, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_USER,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

const HOTKEY_ID: i32 = 1;
const HOTKEY_CLICK_TEST: i32 = 2;
const HOTKEY_SENDINPUT_TEST: i32 = 3;
const HOTKEY_RELATIVE_CLICK: i32 = 4;
const HOTKEY_BRIGHTNESS_TEST: i32 = 5;
const WM_TRAYICON: u32 = WM_USER + 1;
const MENU_EXIT: usize = 1001;

/// Logs a message to both console and log file with timestamp.
pub fn log(msg: &str) {
    let timestamp = Local::now().format("%H:%M:%S%.3f");
    let line = format!("[{}] {}\n", timestamp, msg);
    print!("{}", line);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("gakumas_screenshot.log")
    {
        let _ = file.write_all(line.as_bytes());
    }
}

static mut MAIN_HWND: HWND = HWND(std::ptr::null_mut());

fn main() -> Result<()> {
    unsafe {
        windows::Win32::System::WinRT::RoInitialize(
            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
        )?
    };

    // Load configuration
    automation::init_config();

    // Create hidden window for message handling
    let hwnd = create_message_window()?;
    unsafe { MAIN_HWND = hwnd };

    // Add system tray icon
    add_tray_icon(hwnd)?;

    // Register global hotkey: Ctrl+Shift+S for screenshot
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_ID,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            0x53, // 'S' key
        )?;
    }

    // Register global hotkey: Ctrl+Shift+F9 for PostMessage click test
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_CLICK_TEST,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            0x78, // VK_F9
        )?;
    }

    // Register global hotkey: Ctrl+Shift+F10 for SendInput click test
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_SENDINPUT_TEST,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            0x79, // VK_F10
        )?;
    }

    // Register global hotkey: Ctrl+Shift+F12 for relative click test
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_RELATIVE_CLICK,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            0x7B, // VK_F12
        )?;
    }

    // Register global hotkey: Ctrl+Shift+F11 for brightness test
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_BRIGHTNESS_TEST,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            0x7A, // VK_F11
        )?;
    }

    log("Gakumas Screenshot Tool started");
    log("Hotkey: Ctrl+Shift+S (screenshot)");
    log("Hotkey: Ctrl+Shift+F9 (PostMessage click test)");
    log("Hotkey: Ctrl+Shift+F10 (SendInput click test - MOVES CURSOR)");
    log("Hotkey: Ctrl+Shift+F11 (brightness test)");
    log("Hotkey: Ctrl+Shift+F12 (relative click test - MOVES CURSOR)");
    log("Right-click tray icon to exit");

    // Message loop
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Cleanup
        let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
        let _ = UnregisterHotKey(hwnd, HOTKEY_CLICK_TEST);
        let _ = UnregisterHotKey(hwnd, HOTKEY_SENDINPUT_TEST);
        let _ = UnregisterHotKey(hwnd, HOTKEY_RELATIVE_CLICK);
        let _ = UnregisterHotKey(hwnd, HOTKEY_BRIGHTNESS_TEST);
        remove_tray_icon(hwnd);
        let _ = DestroyWindow(hwnd);
    }

    Ok(())
}

fn create_message_window() -> Result<HWND> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class_name = w!("GakumasScreenshotClass");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(anyhow!("Failed to register window class"));
        }

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("Gakumas Screenshot"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            hinstance,
            None,
        )?;

        Ok(hwnd)
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_HOTKEY => {
                if wparam.0 as i32 == HOTKEY_ID {
                    log("Hotkey pressed! Capturing...");
                    match capture::capture_gakumas() {
                        Ok(path) => log(&format!("Screenshot saved: {}", path.display())),
                        Err(e) => log(&format!("Capture failed: {}", e)),
                    }
                } else if wparam.0 as i32 == HOTKEY_CLICK_TEST {
                    log("PostMessage click test hotkey pressed!");
                    match automation::test_postmessage_click() {
                        Ok(()) => log("PostMessage click test completed"),
                        Err(e) => log(&format!("PostMessage click test failed: {}", e)),
                    }
                } else if wparam.0 as i32 == HOTKEY_SENDINPUT_TEST {
                    log("SendInput click test hotkey pressed!");
                    match automation::test_sendinput_click() {
                        Ok(()) => log("SendInput click test completed"),
                        Err(e) => log(&format!("SendInput click test failed: {}", e)),
                    }
                } else if wparam.0 as i32 == HOTKEY_RELATIVE_CLICK {
                    log("Relative click test hotkey pressed!");
                    let config = automation::get_config();
                    match capture::find_gakumas_window() {
                        Ok(game_hwnd) => {
                            let pos = &config.test_click_position;
                            match automation::click_at_relative(game_hwnd, pos.x, pos.y) {
                                Ok(()) => log("Relative click test completed"),
                                Err(e) => log(&format!("Relative click test failed: {}", e)),
                            }
                        }
                        Err(e) => log(&format!("Could not find game window: {}", e)),
                    }
                } else if wparam.0 as i32 == HOTKEY_BRIGHTNESS_TEST {
                    log("Brightness test hotkey pressed!");
                    let config = automation::get_config();
                    match capture::find_gakumas_window() {
                        Ok(game_hwnd) => {
                            log("Capturing region for brightness test...");
                            match automation::measure_region_brightness(game_hwnd, config) {
                                Ok(brightness) => {
                                    log(&format!("Region brightness: {:.2}", brightness));
                                    log(&format!(
                                        "Threshold is {:.2} (current {} threshold)",
                                        config.brightness_threshold,
                                        if brightness > config.brightness_threshold {
                                            "EXCEEDS"
                                        } else {
                                            "BELOW"
                                        }
                                    ));
                                }
                                Err(e) => log(&format!("Brightness test failed: {}", e)),
                            }
                        }
                        Err(e) => log(&format!("Could not find game window: {}", e)),
                    }
                }
                LRESULT(0)
            }
            WM_TRAYICON => {
                let event = (lparam.0 & 0xFFFF) as u32;
                match event {
                    WM_RBUTTONUP => {
                        show_context_menu(hwnd);
                    }
                    WM_LBUTTONDBLCLK => {
                        // Double-click could trigger capture or show status
                        log("Tray icon double-clicked");
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let cmd = wparam.0 & 0xFFFF;
                if cmd == MENU_EXIT {
                    log("Exit requested");
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn add_tray_icon(hwnd: HWND) -> Result<()> {
    unsafe {
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: LoadIconW(None, IDI_APPLICATION)?,
            ..Default::default()
        };

        // Set tooltip
        let tip = "Gakumas Screenshot (Ctrl+Shift+S)";
        let tip_wide: Vec<u16> = tip.encode_utf16().chain(std::iter::once(0)).collect();
        let len = tip_wide.len().min(nid.szTip.len());
        nid.szTip[..len].copy_from_slice(&tip_wide[..len]);

        if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
            return Err(anyhow!("Failed to add tray icon"));
        }

        Ok(())
    }
}

fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

fn show_context_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap();

        let exit_text = w!("Exit");
        let _ = InsertMenuW(menu, 0, MF_BYPOSITION | MF_STRING, MENU_EXIT, exit_text);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        // Required for the menu to work properly
        let _ = SetForegroundWindow(hwnd);

        let _ = TrackPopupMenu(
            menu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );

        let _ = DestroyMenu(menu);
    }
}
