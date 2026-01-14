# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## ExecPlans

When writing complex features or significant refactors, use an ExecPlan (as described in docs/PLANS.md) from design to implementation.


## Project Overview

Windows screenshot tool that captures the client area of `gakumas.exe` using Windows Graphics Capture API. Runs as a system tray application with global hotkey support.

## COMMIT DISCIPLINE
- Follow Git-flow workflow to manage the branches
- Use small, frequent commits rather than large, infrequent ones
- Only add and commit affected files. Keep untracked other files as are
- Never add Claude Code attribution in commit

## Build Commands

```powershell
# Build release (optimized with LTO)
cargo build --release

# Run
.\target\release\gakumas-screenshot.exe
```

## Architecture

Single-file Rust application (`src/main.rs`) with these key components:

- **Window Management**: Hidden message window for hotkey/tray events, system tray icon with context menu
- **Window Discovery**: `EnumWindows` + `QueryFullProcessImageNameW` to find target process by executable name
- **Screen Capture**: Windows Graphics Capture (WGC) API via `IGraphicsCaptureItemInterop::CreateForWindow`
- **GPU Pipeline**: D3D11 device creates staging texture, copies captured frame, maps for CPU read
- **Image Processing**: Crops to client area (excludes title bar/borders), converts BGRA→RGBA, saves as PNG

## Key Constants and Hotkeys

- Process matching: exact match `"gakumas.exe"` (case-insensitive)
- `HOTKEY_ID` (1): Ctrl+Shift+S - Screenshot
- `HOTKEY_CLICK_TEST` (2): Ctrl+Shift+F9 - PostMessage click test
- `HOTKEY_SENDINPUT_TEST` (3): Ctrl+Shift+F10 - SendInput click test
- Output: `gakumas_YYYYMMDD_HHMMSS.png` in current directory
- Log: `gakumas_screenshot.log` in current directory

## Windows API Notes

- Uses Rust 2024 edition requiring explicit `unsafe` blocks inside `unsafe fn`
- `EnumWindows` returns FALSE when callback stops early - don't treat as error
- `windows` crate v0.58 feature flags must match APIs used (see Cargo.toml)
- `SendInput` with `SetForegroundWindow` is required for game input (PostMessage is ignored)
- Must run as Administrator if game runs elevated (UIPI restriction)

## Design Constraints

- **Admin privileges required**: The executable has a Windows manifest (`gakumas-screenshot.exe.manifest`) that requires administrator elevation. This is necessary for `SendInput` to work with elevated game processes.
- **No command-line arguments**: This is a system tray application, not a CLI tool. All functionality should be accessed via tray menu, hotkeys, or config file. Do not add command-line argument handling.
- **Testing limitations**: Unit tests requiring the binary cannot run from `cargo test` due to the admin manifest. Test functionality manually via tray menu or create separate test utilities if needed.

## Roadmap

See `docs/ROADMAP_AUTOMATION.md` for the full automation feature roadmap including:
- UI automation (clicking buttons)
- OCR integration (Tesseract)
- Statistics and visualization
