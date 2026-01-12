use anyhow::{anyhow, Context, Result};
use chrono::Local;
use image::{ImageBuffer, Rgba};
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use windows::core::{w, Interface};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    EnumWindows, GetClientRect, GetCursorPos, GetMessageW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, InsertMenuW, IsWindowVisible, LoadIconW,
    PostQuitMessage, RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDI_APPLICATION, MF_BYPOSITION, MF_STRING, MSG,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_COMMAND, WM_DESTROY, WM_HOTKEY,
    WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

const HOTKEY_ID: i32 = 1;
const TARGET_PROCESS: &str = "gakumas";
const WM_TRAYICON: u32 = WM_USER + 1;
const MENU_EXIT: usize = 1001;

fn log(msg: &str) {
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

    // Create hidden window for message handling
    let hwnd = create_message_window()?;
    unsafe { MAIN_HWND = hwnd };

    // Add system tray icon
    add_tray_icon(hwnd)?;

    // Register global hotkey: Ctrl+Shift+S
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_ID,
            MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
            0x53, // 'S' key
        )?;
    }

    log("Gakumas Screenshot Tool started");
    log("Hotkey: Ctrl+Shift+S");
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
                    match capture_gakumas() {
                        Ok(path) => log(&format!("Screenshot saved: {}", path.display())),
                        Err(e) => log(&format!("Capture failed: {}", e)),
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

fn capture_gakumas() -> Result<PathBuf> {
    log("Starting capture...");

    let hwnd = find_gakumas_window()?;
    log(&format!("Window handle: {:?}", hwnd));

    let (client_rect, client_offset) = get_client_area_info(hwnd)?;
    let client_width = client_rect.right - client_rect.left;
    let client_height = client_rect.bottom - client_rect.top;
    log(&format!(
        "Client area: {}x{} at offset ({}, {})",
        client_width, client_height, client_offset.x, client_offset.y
    ));

    // Create D3D11 device
    log("Creating D3D11 device...");
    let (device, context) = create_d3d11_device()?;
    log("D3D11 device created");

    // Create capture item from window
    log("Creating capture item...");
    let item = create_capture_item(hwnd)?;
    log("Capture item created");
    let size = item.Size()?;
    log(&format!("Capture size: {}x{}", size.Width, size.Height));

    // Create frame pool
    log("Creating Direct3D device wrapper...");
    let d3d_device = create_direct3d_device(&device)?;
    log("Creating frame pool...");
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &d3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )?;

    // Create capture session
    log("Creating capture session...");
    let session = frame_pool.CreateCaptureSession(&item)?;
    log("Capture session created");

    // Set up frame arrival handling
    let frame_arrived = Arc::new(AtomicBool::new(false));
    let frame_arrived_clone = frame_arrived.clone();

    frame_pool.FrameArrived(&TypedEventHandler::new(
        move |_pool: &Option<Direct3D11CaptureFramePool>, _| {
            frame_arrived_clone.store(true, Ordering::SeqCst);
            Ok(())
        },
    ))?;

    // Start capture
    log("Starting capture session...");
    session.StartCapture()?;
    log("Capture started, waiting for frame...");

    // Wait for frame
    let start = std::time::Instant::now();
    while !frame_arrived.load(Ordering::SeqCst) {
        if start.elapsed().as_secs() > 5 {
            return Err(anyhow!("Timeout waiting for frame"));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    log("Frame arrived");

    // Get the frame
    log("Getting frame...");
    let frame = frame_pool.TryGetNextFrame()?;
    log("Got frame");
    let surface = frame.Surface()?;

    // Get the D3D11 texture from the surface
    let access: windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess =
        surface.cast()?;
    let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };

    // Get texture description
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };

    // Create staging texture for CPU read
    let staging_desc = D3D11_TEXTURE2D_DESC {
        Width: desc.Width,
        Height: desc.Height,
        MipLevels: 1,
        ArraySize: 1,
        Format: desc.Format,
        SampleDesc: desc.SampleDesc,
        Usage: D3D11_USAGE_STAGING,
        BindFlags: Default::default(),
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: Default::default(),
    };

    let staging_texture = unsafe {
        let mut staging: Option<ID3D11Texture2D> = None;
        device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
        staging.ok_or_else(|| anyhow!("Failed to create staging texture"))?
    };

    // Copy to staging texture
    unsafe {
        context.CopyResource(
            &staging_texture.cast::<ID3D11Resource>()?,
            &texture.cast::<ID3D11Resource>()?,
        );
    }

    // Map the staging texture
    let mapped = unsafe {
        let mut mapped = Default::default();
        context.Map(
            &staging_texture.cast::<ID3D11Resource>()?,
            0,
            D3D11_MAP_READ,
            0,
            Some(&mut mapped),
        )?;
        mapped
    };

    // Calculate crop parameters
    let crop_x = client_offset.x as u32;
    let crop_y = client_offset.y as u32;
    let crop_width = client_width as u32;
    let crop_height = client_height as u32;

    log(&format!(
        "Cropping from ({}, {}) size {}x{}",
        crop_x, crop_y, crop_width, crop_height
    ));

    // Create image from mapped data (cropped to client area)
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(crop_width, crop_height);

    let src_data = unsafe {
        std::slice::from_raw_parts(
            mapped.pData as *const u8,
            (mapped.RowPitch * desc.Height) as usize,
        )
    };
    let row_pitch = mapped.RowPitch as usize;

    for y in 0..crop_height {
        let src_y = (crop_y + y) as usize;
        if src_y >= desc.Height as usize {
            break;
        }
        for x in 0..crop_width {
            let src_x = (crop_x + x) as usize;
            if src_x >= desc.Width as usize {
                break;
            }
            let offset = src_y * row_pitch + src_x * 4;
            // BGRA -> RGBA
            let b = src_data[offset];
            let g = src_data[offset + 1];
            let r = src_data[offset + 2];
            let a = src_data[offset + 3];
            img.put_pixel(x, y, Rgba([r, g, b, a]));
        }
    }

    // Unmap
    unsafe {
        context.Unmap(&staging_texture.cast::<ID3D11Resource>()?, 0);
    }

    // Stop capture
    session.Close()?;
    frame_pool.Close()?;

    // Save to file
    log("Saving image...");
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("gakumas_{}.png", timestamp);
    let path = std::env::current_dir()?.join(&filename);

    img.save(&path)?;
    log(&format!("Saved to {}", path.display()));

    Ok(path)
}

fn find_gakumas_window() -> Result<HWND> {
    struct EnumData {
        hwnd: Option<HWND>,
        process_name: Option<String>,
        debug: bool,
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let data = &mut *(lparam.0 as *mut EnumData);

            // Skip invisible windows
            if !IsWindowVisible(hwnd).as_bool() {
                return TRUE;
            }

            // Get window title for debug
            let title_len = GetWindowTextLengthW(hwnd);
            let title = if title_len > 0 {
                let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
                GetWindowTextW(hwnd, &mut title_buf);
                OsString::from_wide(&title_buf[..title_len as usize])
                    .to_string_lossy()
                    .to_string()
            } else {
                String::new()
            };

            // Skip windows without title (usually not main windows)
            if title.is_empty() {
                return TRUE;
            }

            // Get process ID from window
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
            if process_id == 0 {
                return TRUE;
            }

            // Open the process to get its name
            let process_handle =
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id);
            let Ok(process_handle) = process_handle else {
                if data.debug {
                    log(&format!(
                        "  [{}] \"{}\" - failed to open process",
                        process_id, title
                    ));
                }
                return TRUE;
            };

            // Get process executable name
            let mut name_buf: Vec<u16> = vec![0; 1024];
            let mut len = name_buf.len() as u32;
            let result = QueryFullProcessImageNameW(
                process_handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut len,
            );
            let _ = windows::Win32::Foundation::CloseHandle(process_handle);

            if result.is_err() || len == 0 {
                if data.debug {
                    log(&format!(
                        "  [{}] \"{}\" - failed to get process name",
                        process_id, title
                    ));
                }
                return TRUE;
            }

            let full_path = OsString::from_wide(&name_buf[..len as usize])
                .to_string_lossy()
                .to_string();
            // Extract just the filename from the full path
            let process_name = full_path
                .rsplit('\\')
                .next()
                .unwrap_or(&full_path)
                .to_string();
            let process_name_lower = process_name.to_lowercase();

            if data.debug {
                log(&format!(
                    "  [{}] {} - \"{}\"",
                    process_id, process_name, title
                ));
            }

            // Check if this is gakumas.exe
            if process_name_lower.contains(TARGET_PROCESS) {
                data.hwnd = Some(hwnd);
                data.process_name = Some(process_name);
                return BOOL(0); // Stop enumeration
            }

            TRUE
        }
    }

    log("Searching for gakumas.exe window...");
    log("Listing visible windows:");
    let mut data = EnumData {
        hwnd: None,
        process_name: None,
        debug: true,
    };
    unsafe {
        // Don't use ? here - EnumWindows returns FALSE when callback stops it early,
        // which is expected behavior, not an error
        let _ = EnumWindows(Some(enum_callback), LPARAM(&mut data as *mut _ as isize));
    }

    if let Some(name) = &data.process_name {
        log(&format!("Found process: \"{}\"", name));
    }

    data.hwnd
        .ok_or_else(|| anyhow!("Could not find gakumas.exe window. Is the game running?"))
}

fn get_client_area_info(hwnd: HWND) -> Result<(RECT, POINT)> {
    let mut client_rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client_rect)? };

    // Get the offset of client area relative to window
    let mut client_origin = POINT { x: 0, y: 0 };
    unsafe {
        if !ClientToScreen(hwnd, &mut client_origin).as_bool() {
            return Err(anyhow!("ClientToScreen failed"));
        }
    }

    let mut window_rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut window_rect)? };

    let offset = POINT {
        x: client_origin.x - window_rect.left,
        y: client_origin.y - window_rect.top,
    };

    Ok((client_rect, offset))
}

fn create_d3d11_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;

    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
    }

    Ok((
        device.ok_or_else(|| anyhow!("Failed to create D3D11 device"))?,
        context.ok_or_else(|| anyhow!("Failed to create D3D11 context"))?,
    ))
}

fn create_direct3d_device(
    device: &ID3D11Device,
) -> Result<windows::Graphics::DirectX::Direct3D11::IDirect3DDevice> {
    let dxgi_device: windows::Win32::Graphics::Dxgi::IDXGIDevice = device.cast()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)? };
    inspectable
        .cast()
        .context("Failed to cast to IDirect3DDevice")
}

fn create_capture_item(hwnd: HWND) -> Result<GraphicsCaptureItem> {
    let class_name = windows::core::h!("Windows.Graphics.Capture.GraphicsCaptureItem");
    log("Getting activation factory...");
    let interop: IGraphicsCaptureItemInterop = unsafe {
        windows::Win32::System::WinRT::RoGetActivationFactory(class_name)
            .context("Failed to get IGraphicsCaptureItemInterop")?
    };
    log("Got activation factory");

    log(&format!("Creating capture item for window {:?}...", hwnd));
    unsafe {
        interop
            .CreateForWindow(hwnd)
            .context("Failed to create capture item for window")
    }
}
