# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Windows screenshot tool that captures the client area of `gakumas.exe` using Windows Graphics Capture API. Runs as a system tray application with global hotkey support.

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

## Key Constants

- `TARGET_PROCESS`: "gakumas" - substring match against process executable name
- `HOTKEY_ID`: Ctrl+Shift+S (0x53)
- Output: `gakumas_YYYYMMDD_HHMMSS.png` in current directory
- Log: `gakumas_screenshot.log` in current directory

## Windows API Notes

- Uses Rust 2024 edition requiring explicit `unsafe` blocks inside `unsafe fn`
- `EnumWindows` returns FALSE when callback stops early - don't treat as error
- `windows` crate v0.58 feature flags must match APIs used (see Cargo.toml)
