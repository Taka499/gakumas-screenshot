use anyhow::{anyhow, Result};
use regex::Regex;

use super::engine::OcrLine;
use crate::log;

/// Pattern to match score-like words:
/// - Plain numbers: 12345
/// - Numbers with comma separators: 12,345 or 1,234,567
/// - Numbers with period separators: 12.345
/// - Dashes: -- or — (indicating zero or missing score)
const SCORE_PATTERN: &str = r"^((\d+[,.])*\d+|[—\-]+)$";

/// Minimum confidence threshold for accepting OCR lines
const MIN_CONFIDENCE: f32 = 60.0;

/// Extracts 9 scores from OCR output using pattern matching.
/// Returns [[u32; 3]; 3] representing [stage][breakdown] scores.
pub fn extract_scores(lines: &[OcrLine]) -> Result<[[u32; 3]; 3]> {
    let score_regex = Regex::new(SCORE_PATTERN)?;
    let mut scores: Vec<[u32; 3]> = Vec::new();

    for line in lines {
        // Skip low-confidence results
        if line.confidence < MIN_CONFIDENCE {
            continue;
        }

        // Filter words to only score-like patterns
        let score_words: Vec<&str> = line
            .words
            .iter()
            .map(|w| w.text.as_str())
            .filter(|text| score_regex.is_match(text))
            .collect();

        // Only accept lines with exactly 3 score words
        if score_words.len() != 3 {
            continue;
        }

        // Parse the three scores
        let mut stage_scores = [0u32; 3];
        for (i, word) in score_words.iter().enumerate() {
            stage_scores[i] = parse_score(word)?;
        }

        log(&format!(
            "Found score line: {:?} (conf: {:.0}%)",
            stage_scores, line.confidence
        ));

        scores.push(stage_scores);

        // Stop after finding 3 stages
        if scores.len() == 3 {
            break;
        }
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
    // Handle dashes as zero
    if text.chars().all(|c| c == '-' || c == '—') {
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
}
