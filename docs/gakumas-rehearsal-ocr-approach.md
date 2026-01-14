# Gakumas Tools: Rehearsal Score OCR Approach

This document summarizes how [gakumas-tools](https://github.com/surisuririsu/gakumas-tools) extracts rehearsal scores from screenshots using OCR.

## Overview

The system extracts the three breakdown scores for each of the three stages from a rehearsal result screen. Rather than cropping specific regions, it processes the entire image and uses color filtering combined with pattern matching to identify score lines.

**Output format:** A 3×3 array of scores (3 stages × 3 criteria per stage)

## Pipeline

```
Input Image
    ↓
Color Thresholding (isolate bright pixels)
    ↓
Full-Image OCR (Tesseract.js)
    ↓
Line Filtering (pattern matching)
    ↓
Score Extraction
```

## Step 1: Color-Based Preprocessing

The `getWhiteCanvas()` function converts the image to a binary (black/white) format optimized for OCR.

```javascript
export function getWhiteCanvas(img, threshold = 252) {
  return getPreprocessedCanvas(img, (r, g, b) =>
    [r, g, b].every((v) => v > threshold)
  );
}
```

**Logic:**
- Pixels where R, G, and B are ALL above the threshold → **black** (preserved as text)
- All other pixels → **white** (erased as background)

**Threshold values:**
| Source | Threshold | Rationale |
|--------|-----------|-----------|
| Image files | `190` | Screenshots have clean, consistent colors |
| Video frames | `160` | Compression artifacts reduce brightness; needs looser threshold |

**Why this works:** The score numbers in the game UI are displayed in a bright white/cream color (RGB ~240-255), while the background, character portraits, and other UI elements are darker. This color difference allows clean isolation of the target text.

## Step 2: Full-Image OCR

```javascript
const result = await worker.recognize(whiteCanvas, {}, { blocks: true });
```

- Uses **Tesseract.js** with the English language model (`eng`)
- Processes the **entire preprocessed image** (no cropping)
- Returns structured data including text blocks, paragraphs, lines, and words with confidence scores

## Step 3: Line Extraction

```javascript
export function extractLines(result) {
  return result.data.blocks
    .map((block) => block.paragraphs.map((paragraph) => paragraph.lines))
    .flat(2);
}
```

Flattens Tesseract's hierarchical output (blocks → paragraphs → lines) into a simple array of lines.

## Step 4: Score Filtering and Extraction

```javascript
export function extractScores(result) {
  let scores = [];
  const lines = extractLines(result);

  for (let i in lines) {
    const line = lines[i];

    // Skip low-confidence OCR results
    if (line.confidence < 60) continue;

    // Filter words to only number-like patterns
    let words = line.words
      .map((word) => word.text)
      .filter((word) => /^((\d+[,\.])?\d+|[—\-]+)$/.test(word));

    // Only accept lines with exactly 3 number words
    if (words.length != 3) continue;

    // Parse numbers, removing commas/periods
    const stageScores = words.map(
      (word) => parseInt(word.replaceAll(/[^\d]/g, ""), 10) || ""
    );

    scores.push(stageScores);

    // Stop after finding 3 stages
    if (scores.length == 3) break;
  }

  return scores;
}
```

**Filtering criteria:**
1. **Confidence threshold:** Lines with <60% OCR confidence are discarded
2. **Word pattern:** Only keeps words matching the regex `^((\d+[,\.])?\d+|[—\-]+)$`
   - Matches: `50,139`, `148808`, `1.234`, `--`
   - Rejects: `ステージ`, `Pt`, `総合力`, etc.
3. **Word count:** Lines must contain exactly 3 matching words
4. **Early termination:** Stops after finding 3 valid lines (one per stage)

## Design Advantages

1. **Resolution independent:** No hardcoded pixel coordinates; works with any screen size
2. **Aspect ratio tolerant:** Pattern matching adapts to different layouts
3. **Noise resistant:** Confidence filtering and strict pattern matching reduce false positives
4. **Simple preprocessing:** Single-threshold binarization is fast and effective for this UI style

## Limitations

- Assumes scores are displayed in a bright color against a darker background
- Requires the three breakdown scores per stage to appear on the same line
- May fail if OCR confidence drops below 60% (e.g., very low resolution images)
- Regex pattern assumes comma or period as thousand separators

## File Structure

```
gakumas-tools/utils/imageProcessing/
├── common.js      # Shared utilities (image loading, preprocessing, line extraction)
└── rehearsal.js   # Rehearsal-specific score extraction
```

## Dependencies

- **Tesseract.js** — Client-side OCR engine
- Browser Canvas API (or `OffscreenCanvas` for web workers)
