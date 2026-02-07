Debug OCR crop regions for a screenshot to diagnose score extraction issues.

Usage: /debug-ocr <screenshot_path>

Steps:
1. Run the debug script:
   ```
   uv run scripts/debug_ocr_regions.py $ARGUMENTS --config config.json
   ```
2. Read all generated images from the project root `debug/` folder using the Read tool:
   - `debug/original.png` — copy of the original screenshot for comparison
   - `debug/regions_overlay.png` — full screenshot with colored region rectangles
   - `debug/stage1_crop.png`, `debug/stage2_crop.png`, `debug/stage3_crop.png` — raw cropped sub-images
   - `debug/stage1_threshold.png`, `debug/stage2_threshold.png`, `debug/stage3_threshold.png` — binary thresholded images
3. Report:
   - The pixel coordinates of each stage's crop region
   - Whether each region correctly covers the score row (no clipping of digits)
   - Whether the threshold images show clean black text on white background
   - Any alignment issues or suggestions for adjusting `score_regions` in config.json
