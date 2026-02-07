use anyhow::{anyhow, Result};
use regex::Regex;

use super::engine::OcrLine;
use crate::log;

/// Pattern to match score-like words:
/// - Plain numbers: 12345
/// - Numbers with comma separators: 12,345 or 1,234,567
/// - Numbers with period separators: 12.345
/// - Dashes: -- or — or ー or 一 or – or ― or ─ (indicating zero or missing score)
const SCORE_PATTERN: &str =
    r"^((\d+[,.])*\d+|[\-\u{2014}\u{2013}\u{2015}\u{2500}\u{30FC}\u{4E00}]+)$";

/// Minimum confidence threshold for accepting OCR lines
const MIN_CONFIDENCE: f32 = 60.0;

/// Returns true if the character is a dash-like character used for missing scores.
fn is_dash_char(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{2014}' // em-dash —
            | '\u{2013}' // en-dash –
            | '\u{2015}' // horizontal bar ―
            | '\u{2500}' // box drawing horizontal ─
            | '\u{30FC}' // katakana prolonged sound mark ー
            | '\u{4E00}' // CJK "one" 一
    )
}

/// Returns true if the text looks like a garbled/dropped dash from OCR.
/// Matches short strings (1-3 chars) composed of common dash-like or
/// OCR-misread characters (e.g., "I", "l", "|", "_", "~").
fn is_dash_like(text: &str) -> bool {
    let len = text.chars().count();
    if len == 0 || len > 3 {
        return false;
    }
    text.chars().all(|c| {
        is_dash_char(c)
            || matches!(
                c,
                'I' | 'l' | '|' | '_' | '~' | '=' | '/' | '\\' | '(' | ')' | '[' | ']'
            )
    })
}

/// Extracts per-character scores from a single cropped stage region.
///
/// Collects all words matching SCORE_PATTERN from the OCR output, filters out
/// noise (scores < 100), and maps them left-to-right. Since blank characters (ー)
/// are always on the right side, missing slots are padded with 0 on the right.
///
/// Returns an error if no scores are found (each stage has at least 1 character).
pub fn extract_single_stage(lines: &[OcrLine]) -> Result<[u32; 3]> {
    let score_regex = Regex::new(SCORE_PATTERN)?;

    let mut scores: Vec<u32> = Vec::new();

    for line in lines {
        for word in &line.words {
            if score_regex.is_match(&word.text) {
                let val = parse_score(&word.text)?;
                // Filter noise: real per-character scores are thousands+
                if val >= 100 {
                    scores.push(val);
                }
                // val == 0 means dash → skip (blank character, don't count)
            }
        }
    }

    if scores.is_empty() {
        return Err(anyhow!("No scores found in cropped stage region"));
    }

    // Map left-to-right, pad missing positions with 0
    let mut result = [0u32; 3];
    for (i, &s) in scores.iter().take(3).enumerate() {
        result[i] = s;
    }

    log(&format!(
        "Stage scores: {:?} (found {} words)",
        result,
        scores.len()
    ));

    Ok(result)
}

/// Extracts 9 scores from OCR output using pattern matching.
/// Returns [[u32; 3]; 3] representing [stage][breakdown] scores.
///
/// Uses a multi-pass approach to handle cases where dash characters (ー)
/// indicating missing scores are garbled or dropped by OCR:
/// - Pass 1: Strict match (exactly 3 score words per line)
/// - Pass 2: Accept lines with score words + dash-like short words (total >= 3)
/// - Pass 3: Accept lines with 1-2 score words (dashes completely dropped), pad with 0
pub fn extract_scores(lines: &[OcrLine]) -> Result<[[u32; 3]; 3]> {
    let score_regex = Regex::new(SCORE_PATTERN)?;
    let mut scores: Vec<[u32; 3]> = Vec::new();
    let mut used_lines: Vec<usize> = Vec::new();

    // Pass 1: Strict match - exactly 3 score words per line
    for (idx, line) in lines.iter().enumerate() {
        if line.confidence < MIN_CONFIDENCE {
            continue;
        }

        let score_words: Vec<&str> = line
            .words
            .iter()
            .map(|w| w.text.as_str())
            .filter(|text| score_regex.is_match(text))
            .collect();

        if score_words.len() != 3 {
            continue;
        }

        let mut stage_scores = [0u32; 3];
        for (i, word) in score_words.iter().enumerate() {
            stage_scores[i] = parse_score(word)?;
        }

        log(&format!(
            "Found score line: {:?} (conf: {:.0}%)",
            stage_scores, line.confidence
        ));

        scores.push(stage_scores);
        used_lines.push(idx);

        if scores.len() == 3 {
            break;
        }
    }

    if scores.len() == 3 {
        return Ok([scores[0], scores[1], scores[2]]);
    }

    // Pass 2: Accept lines with score words + dash-like words (total >= 3)
    log(&format!(
        "Pass 1 found {} stages, trying pass 2 (dash-like fallback)...",
        scores.len()
    ));

    for (idx, line) in lines.iter().enumerate() {
        if scores.len() == 3 {
            break;
        }
        if used_lines.contains(&idx) || line.confidence < MIN_CONFIDENCE {
            continue;
        }

        let mut stage_scores = [0u32; 3];
        let mut pos = 0;

        for word in &line.words {
            if pos >= 3 {
                break;
            }
            let text = word.text.as_str();
            if score_regex.is_match(text) {
                stage_scores[pos] = parse_score(text)?;
                pos += 1;
            } else if is_dash_like(text) {
                // Treat garbled dash as zero
                stage_scores[pos] = 0;
                pos += 1;
            }
        }

        if pos == 3 {
            log(&format!(
                "Found score line (pass 2): {:?} (conf: {:.0}%)",
                stage_scores, line.confidence
            ));
            scores.push(stage_scores);
            used_lines.push(idx);
        }
    }

    if scores.len() == 3 {
        return Ok([scores[0], scores[1], scores[2]]);
    }

    // Pass 3: Accept lines with 1-2 score words (dashes completely dropped)
    // Only look at lines after the last matched stage to reduce false positives
    let search_start = used_lines.iter().max().map(|&i| i + 1).unwrap_or(0);

    log(&format!(
        "Pass 2 found {} stages, trying pass 3 (partial lines from line {})...",
        scores.len(),
        search_start
    ));

    for (idx, line) in lines.iter().enumerate() {
        if scores.len() == 3 {
            break;
        }
        if idx < search_start || used_lines.contains(&idx) || line.confidence < MIN_CONFIDENCE {
            continue;
        }

        let score_words: Vec<&str> = line
            .words
            .iter()
            .map(|w| w.text.as_str())
            .filter(|text| score_regex.is_match(text))
            .collect();

        // Accept lines with 1-2 score words - these are lines where dashes were dropped
        if score_words.is_empty() || score_words.len() > 3 {
            continue;
        }

        let mut stage_scores = [0u32; 3];
        for (i, word) in score_words.iter().enumerate() {
            stage_scores[i] = parse_score(word)?;
        }
        // Remaining positions stay as 0 (missing dashes)

        log(&format!(
            "Found score line (pass 3, {} of 3 words): {:?} (conf: {:.0}%)",
            score_words.len(),
            stage_scores,
            line.confidence
        ));

        scores.push(stage_scores);
        used_lines.push(idx);
    }

    if scores.len() < 3 {
        return Err(anyhow!(
            "Could not find all 3 stage scores. Found {} stages.",
            scores.len()
        ));
    }

    Ok([scores[0], scores[1], scores[2]])
}

/// Parses a single score string, removing commas, periods, and whitespace.
/// Dashes are treated as zero.
pub fn parse_score(text: &str) -> Result<u32> {
    // Handle dashes as zero (ASCII hyphen, em-dash, en-dash, horizontal bar,
    // box drawing horizontal, katakana prolonged sound mark, CJK unified ideograph "one")
    if text
        .chars()
        .all(|c| is_dash_char(c))
    {
        return Ok(0);
    }

    // Remove all non-digit characters
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return Err(anyhow!("No digits found in score: {}", text));
    }

    digits
        .parse::<u32>()
        .map_err(|e| anyhow!("Failed to parse score '{}': {}", text, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::engine::OcrWord;

    fn make_line(words: &[&str], confidence: f32) -> OcrLine {
        OcrLine {
            text: words.join(" "),
            words: words
                .iter()
                .map(|w| OcrWord {
                    text: w.to_string(),
                    confidence,
                })
                .collect(),
            confidence,
        }
    }

    #[test]
    fn test_parse_score() {
        assert_eq!(parse_score("12345").unwrap(), 12345);
        assert_eq!(parse_score("12,345").unwrap(), 12345);
        assert_eq!(parse_score("1,234,567").unwrap(), 1234567);
        assert_eq!(parse_score("12.345").unwrap(), 12345);
        assert_eq!(parse_score("--").unwrap(), 0);
        assert_eq!(parse_score("—").unwrap(), 0);
        // Japanese/Unicode dash characters
        assert_eq!(parse_score("ー").unwrap(), 0);
        assert_eq!(parse_score("一").unwrap(), 0);
        assert_eq!(parse_score("–").unwrap(), 0);
        assert_eq!(parse_score("―").unwrap(), 0);
        assert_eq!(parse_score("─").unwrap(), 0);
        assert_eq!(parse_score("ーー").unwrap(), 0);
    }

    #[test]
    fn test_is_dash_like() {
        assert!(is_dash_like("I"));
        assert!(is_dash_like("l"));
        assert!(is_dash_like("|"));
        assert!(is_dash_like("_"));
        assert!(is_dash_like("Il"));
        assert!(!is_dash_like("1234"));
        assert!(!is_dash_like(""));
        assert!(!is_dash_like("hello"));
    }

    #[test]
    fn test_extract_single_stage_three_scores() {
        let lines = vec![make_line(&["12345", "23456", "34567"], 90.0)];
        let result = extract_single_stage(&lines).unwrap();
        assert_eq!(result, [12345, 23456, 34567]);
    }

    #[test]
    fn test_extract_single_stage_two_scores_one_dash() {
        let lines = vec![make_line(&["12345", "23456"], 90.0)];
        let result = extract_single_stage(&lines).unwrap();
        assert_eq!(result, [12345, 23456, 0]);
    }

    #[test]
    fn test_extract_single_stage_one_score_two_dashes() {
        let lines = vec![make_line(&["12345"], 90.0)];
        let result = extract_single_stage(&lines).unwrap();
        assert_eq!(result, [12345, 0, 0]);
    }

    #[test]
    fn test_extract_single_stage_noise_filtered() {
        // Small numbers (<100) should be filtered as noise
        let lines = vec![make_line(&["12345", "50", "23456"], 90.0)];
        let result = extract_single_stage(&lines).unwrap();
        assert_eq!(result, [12345, 23456, 0]);
    }

    #[test]
    fn test_extract_single_stage_no_scores_error() {
        let lines = vec![make_line(&["50", "30"], 90.0)];
        assert!(extract_single_stage(&lines).is_err());
    }

    #[test]
    fn test_extract_single_stage_dashes_ignored() {
        // Dashes parse to 0, which is < 100, so they're skipped
        let lines = vec![make_line(&["12345", "ー", "23456"], 90.0)];
        let result = extract_single_stage(&lines).unwrap();
        assert_eq!(result, [12345, 23456, 0]);
    }

    #[test]
    fn test_extract_scores_basic() {
        let lines = vec![
            make_line(&["50339", "50796", "70859"], 90.0),
            make_line(&["64997", "168009", "128450"], 90.0),
            make_line(&["122130", "105901", "96776"], 90.0),
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [64997, 168009, 128450]);
        assert_eq!(scores[2], [122130, 105901, 96776]);
    }

    #[test]
    fn test_extract_scores_with_noise() {
        let lines = vec![
            make_line(&["ステージ", "1"], 90.0),          // Should be skipped
            make_line(&["50339", "50796", "70859"], 90.0), // Valid
            make_line(&["Pt"], 90.0),                      // Should be skipped
            make_line(&["64997", "168009", "128450"], 90.0), // Valid
            make_line(&["total:", "500000"], 90.0),        // Should be skipped
            make_line(&["122130", "105901", "96776"], 90.0), // Valid
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [64997, 168009, 128450]);
        assert_eq!(scores[2], [122130, 105901, 96776]);
    }

    #[test]
    fn test_extract_scores_low_confidence_skipped() {
        let lines = vec![
            make_line(&["50339", "50796", "70859"], 50.0), // Low confidence, skipped
            make_line(&["50339", "50796", "70859"], 90.0), // Valid
            make_line(&["64997", "168009", "128450"], 90.0),
            make_line(&["122130", "105901", "96776"], 90.0),
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
    }

    #[test]
    fn test_extract_scores_with_commas() {
        let lines = vec![
            make_line(&["50,339", "50,796", "70,859"], 90.0),
            make_line(&["64,997", "168,009", "128,450"], 90.0),
            make_line(&["122,130", "105,901", "96,776"], 90.0),
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [64997, 168009, 128450]);
        assert_eq!(scores[2], [122130, 105901, 96776]);
    }

    #[test]
    fn test_extract_scores_with_japanese_dashes() {
        // Katakana prolonged sound mark ー recognized as score pattern
        let lines = vec![
            make_line(&["50339", "50796", "ー"], 90.0),
            make_line(&["ー", "168009", "128450"], 90.0),
            make_line(&["122130", "ー", "96776"], 90.0),
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 0]);
        assert_eq!(scores[1], [0, 168009, 128450]);
        assert_eq!(scores[2], [122130, 0, 96776]);
    }

    #[test]
    fn test_extract_scores_pass2_garbled_dashes() {
        // OCR reads dash as "I" or "l" (not matching score pattern)
        let lines = vec![
            make_line(&["50339", "50796", "70859"], 90.0),
            make_line(&["I", "168009", "128450"], 90.0),  // "I" is dash-like
            make_line(&["122130", "l", "96776"], 90.0),    // "l" is dash-like
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [0, 168009, 128450]);
        assert_eq!(scores[2], [122130, 0, 96776]);
    }

    #[test]
    fn test_extract_scores_pass3_dropped_dashes() {
        // OCR completely drops the dash characters, leaving only 1-2 words
        let lines = vec![
            make_line(&["50339", "50796", "70859"], 90.0),
            make_line(&["64997", "168009", "128450"], 90.0),
            make_line(&["122130", "96776"], 90.0), // Only 2 words, dash dropped
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [64997, 168009, 128450]);
        // Dropped dash → remaining scores fill from position 0, rest padded with 0
        assert_eq!(scores[2], [122130, 96776, 0]);
    }

    #[test]
    fn test_extract_scores_all_dashes_dropped() {
        // Worst case: one stage has all dashes dropped (empty line skipped)
        // The line with just 1 score word should be picked up in pass 3
        let lines = vec![
            make_line(&["50339", "50796", "70859"], 90.0),
            make_line(&["64997", "168009", "128450"], 90.0),
            make_line(&["96776"], 90.0), // Only 1 score word
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [64997, 168009, 128450]);
        assert_eq!(scores[2], [96776, 0, 0]);
    }

    #[test]
    fn test_extract_scores_mixed_passes() {
        // Mix of normal, garbled, and dropped dashes across stages
        let lines = vec![
            make_line(&["noise"], 90.0),
            make_line(&["50339", "50796", "70859"], 90.0), // Pass 1
            make_line(&["I", "168009", "I"], 90.0),        // Pass 2 (garbled)
            make_line(&["96776"], 90.0),                    // Pass 3 (dropped)
        ];

        let scores = extract_scores(&lines).unwrap();
        assert_eq!(scores[0], [50339, 50796, 70859]);
        assert_eq!(scores[1], [0, 168009, 0]);
        assert_eq!(scores[2], [96776, 0, 0]);
    }
}
