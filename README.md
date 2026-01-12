# gakumas-screenshot

学園アイドルマスター (gakumas.exe) のクライアント領域をキャプチャするスクリーンショットツール。

## 機能

- **ホットキー**: `Ctrl+Shift+S` でスクリーンショットを撮影
- **クライアント領域のみ**: タイトルバーやウィンドウ枠を除いたゲーム画面のみをキャプチャ
- **システムトレイ**: トレイアイコンから右クリックで終了可能
- **Windows Graphics Capture**: 高性能なキャプチャAPI使用（Windows 10 1803以降対応）

## 使い方

1. `gakumas-screenshot.exe` を起動
2. システムトレイにアイコンが表示される
3. ゲームを起動した状態で `Ctrl+Shift+S` を押す
4. 現在のディレクトリに `gakumas_YYYYMMDD_HHMMSS.png` として保存される
5. 終了するにはトレイアイコンを右クリック → Exit

## ビルド方法

```powershell
cargo build --release
```

出力: `target/release/gakumas-screenshot.exe`

## 動作要件

- Windows 10 バージョン 1803 以降
- gakumas.exe が起動していること

## ログ

動作ログは `gakumas_screenshot.log` に出力されます。

---

# English

Screenshot tool for capturing the client area of Gakuen iDOLM@STER (gakumas.exe).

## Features

- **Hotkey**: Press `Ctrl+Shift+S` to take a screenshot
- **Client area only**: Captures only the game screen, excluding title bar and window borders
- **System tray**: Exit via right-click menu on tray icon
- **Windows Graphics Capture**: Uses high-performance capture API (Windows 10 1803+)

## Usage

1. Run `gakumas-screenshot.exe`
2. An icon appears in the system tray
3. With the game running, press `Ctrl+Shift+S`
4. Screenshot is saved as `gakumas_YYYYMMDD_HHMMSS.png` in the current directory
5. To exit, right-click the tray icon → Exit

## Build

```powershell
cargo build --release
```

Output: `target/release/gakumas-screenshot.exe`

## Requirements

- Windows 10 version 1803 or later
- gakumas.exe must be running

## Logs

Operation logs are written to `gakumas_screenshot.log`.
