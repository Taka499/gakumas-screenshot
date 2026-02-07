# /// script
# requires-python = ">=3.12"
# dependencies = ["pillow"]
# ///
"""Debug tool for visualizing OCR crop regions on gakumas screenshots.

Generates overlay images showing where each stage's score region is cropped,
plus the raw crops and thresholded versions matching the Rust OCR pipeline.

Usage:
    uv run scripts/debug_ocr_regions.py <screenshot_path> [--config <config.json>] [--threshold <190>]
"""

import argparse
import json
import shutil
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

# Colors for each stage overlay (RGBA with semi-transparency)
STAGE_COLORS = [
    (255, 0, 0, 80),    # Red
    (0, 255, 0, 80),    # Green
    (0, 0, 255, 80),    # Blue
]
STAGE_BORDER_COLORS = [
    (255, 0, 0, 255),
    (0, 255, 0, 255),
    (0, 0, 255, 255),
]


def load_config(config_path: Path) -> dict:
    with open(config_path) as f:
        return json.load(f)


def resolve_config(config_arg: str | None, screenshot_path: Path) -> Path:
    """Find config.json: explicit arg > project root > exe dir."""
    if config_arg:
        p = Path(config_arg)
        if p.exists():
            return p
        raise FileNotFoundError(f"Config not found: {p}")

    # Try project root (where this script lives)
    project_root = Path(__file__).resolve().parent.parent
    candidate = project_root / "config.json"
    if candidate.exists():
        return candidate

    # Try next to screenshot
    candidate = screenshot_path.parent / "config.json"
    if candidate.exists():
        return candidate

    raise FileNotFoundError("No config.json found. Use --config to specify path.")


def region_to_pixels(region: dict, img_w: int, img_h: int) -> tuple[int, int, int, int]:
    """Convert relative region to absolute pixel coords (x0, y0, x1, y1).

    Replicates Rust crop_region logic:
        x0 = (region.x * w) as u32, clamped to w
        y0 = (region.y * h) as u32, clamped to h
        rw = (region.width * w) as u32, clamped to w - x0
        rh = (region.height * h) as u32, clamped to h - y0
    """
    x0 = min(int(region["x"] * img_w), img_w)
    y0 = min(int(region["y"] * img_h), img_h)
    rw = min(int(region["width"] * img_w), img_w - x0)
    rh = min(int(region["height"] * img_h), img_h - y0)
    return (x0, y0, x0 + rw, y0 + rh)


def threshold_bright_pixels(img: Image.Image, threshold: int) -> Image.Image:
    """Replicate Rust threshold_bright_pixels: R>t AND G>t AND B>t → black, else → white."""
    rgb = img.convert("RGB")
    pixels = rgb.load()
    w, h = rgb.size
    out = Image.new("L", (w, h), 255)
    out_pixels = out.load()

    for y in range(h):
        for x in range(w):
            r, g, b = pixels[x, y]
            if r > threshold and g > threshold and b > threshold:
                out_pixels[x, y] = 0  # Black (text)
            # else stays 255 (white background)

    return out


def try_load_font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    """Try to load a TrueType font, fall back to default."""
    for name in ["arial.ttf", "Arial.ttf", "DejaVuSans.ttf", "segoeui.ttf"]:
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def create_overlay(img: Image.Image, regions: list[dict]) -> Image.Image:
    """Create full screenshot with semi-transparent colored rectangles and labels."""
    overlay = img.copy().convert("RGBA")
    img_w, img_h = overlay.size

    # Create transparent layer for filled rectangles
    fill_layer = Image.new("RGBA", (img_w, img_h), (0, 0, 0, 0))
    fill_draw = ImageDraw.Draw(fill_layer)

    font = try_load_font(20)
    small_font = try_load_font(14)

    for i, region in enumerate(regions):
        x0, y0, x1, y1 = region_to_pixels(region, img_w, img_h)
        color = STAGE_COLORS[i % len(STAGE_COLORS)]
        border = STAGE_BORDER_COLORS[i % len(STAGE_BORDER_COLORS)]

        # Semi-transparent fill
        fill_draw.rectangle([x0, y0, x1, y1], fill=color)

        # Border on fill layer too
        fill_draw.rectangle([x0, y0, x1, y1], outline=border[:3] + (255,), width=2)

        # Label
        label = f"Stage {i + 1}"
        coords = f"({x0}, {y0}) → ({x1}, {y1})  [{x1 - x0}x{y1 - y0}]"
        label_y = y0 - 24 if y0 > 30 else y1 + 4
        fill_draw.text((x0 + 4, label_y), label, fill=border[:3] + (255,), font=font)
        fill_draw.text((x0 + 4, label_y + 22), coords, fill=(255, 255, 255, 220), font=small_font)

    return Image.alpha_composite(overlay, fill_layer)


def main():
    parser = argparse.ArgumentParser(description="Debug OCR crop regions on gakumas screenshots")
    parser.add_argument("screenshot", type=Path, help="Path to screenshot PNG")
    parser.add_argument("--config", type=str, default=None, help="Path to config.json")
    parser.add_argument("--threshold", type=int, default=None, help="Brightness threshold override")
    args = parser.parse_args()

    screenshot_path = args.screenshot.resolve()
    if not screenshot_path.exists():
        print(f"Error: Screenshot not found: {screenshot_path}", file=sys.stderr)
        sys.exit(1)

    # Load config
    config_path = resolve_config(args.config, screenshot_path)
    config = load_config(config_path)
    print(f"Config: {config_path}")

    score_regions = config.get("score_regions", [])
    if not score_regions:
        print("Error: No score_regions in config", file=sys.stderr)
        sys.exit(1)

    threshold = args.threshold if args.threshold is not None else config.get("ocr_threshold", 190)
    print(f"Threshold: {threshold}")

    # Load screenshot
    img = Image.open(screenshot_path)
    img_w, img_h = img.size
    print(f"Screenshot: {img_w}x{img_h}")

    # Create output directory at project root
    project_root = Path(__file__).resolve().parent.parent
    debug_dir = project_root / "debug"
    debug_dir.mkdir(exist_ok=True)
    print(f"Output: {debug_dir}")
    print()

    # Copy original screenshot for side-by-side comparison
    original_copy = debug_dir / "original.png"
    shutil.copy2(screenshot_path, original_copy)
    print(f"Saved: {original_copy}")

    # Generate overlay
    overlay = create_overlay(img, score_regions)
    overlay_path = debug_dir / "regions_overlay.png"
    overlay.save(overlay_path)
    print(f"Saved: {overlay_path}")

    # Generate per-stage crops and thresholds
    img_rgba = img.convert("RGBA")
    for i, region in enumerate(score_regions):
        stage_num = i + 1
        x0, y0, x1, y1 = region_to_pixels(region, img_w, img_h)
        print(f"\nStage {stage_num}: ({x0}, {y0}) → ({x1}, {y1})  crop={x1 - x0}x{y1 - y0}")

        # Crop
        cropped = img_rgba.crop((x0, y0, x1, y1))
        crop_path = debug_dir / f"stage{stage_num}_crop.png"
        cropped.save(crop_path)
        print(f"  Saved: {crop_path}")

        # Threshold
        thresholded = threshold_bright_pixels(cropped, threshold)
        thresh_path = debug_dir / f"stage{stage_num}_threshold.png"
        thresholded.save(thresh_path)
        print(f"  Saved: {thresh_path}")

    print(f"\nDone. {2 + len(score_regions) * 2} images written to {debug_dir}")


if __name__ == "__main__":
    main()
