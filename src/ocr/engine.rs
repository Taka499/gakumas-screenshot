use anyhow::{anyhow, Result};
use image::{ImageBuffer, Luma};
use std::process::Command;
use tempfile::NamedTempFile;

use super::setup::{find_tesseract_executable, find_tessdata_dir};

/// Represents a line of OCR text with confidence score
#[derive(Debug, Clone)]
pub struct OcrLine {
    pub text: String,
    pub words: Vec<OcrWord>,
    pub confidence: f32,
}

/// Represents a single word from OCR with confidence score
#[derive(Debug, Clone)]
pub struct OcrWord {
    pub text: String,
    pub confidence: f32,
}

/// Runs Tesseract on a preprocessed grayscale image.
/// Returns structured output with lines and confidence scores.
pub fn recognize_image(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<Vec<OcrLine>> {
    let tesseract_exe = find_tesseract_executable()?;
    let tessdata_dir = find_tessdata_dir()?;

    // Save image to temporary file
    let temp_input = NamedTempFile::with_suffix(".png")?;
    img.save(temp_input.path())?;

    // Create temporary output file (Tesseract adds .tsv extension)
    let temp_output = NamedTempFile::new()?;
    let output_base = temp_output.path().to_string_lossy().to_string();

    // Run Tesseract with TSV output for structured data
    let output = Command::new(&tesseract_exe)
        .arg(temp_input.path())
        .arg(&output_base)
        .arg("--tessdata-dir")
        .arg(&tessdata_dir)
        .arg("-l")
        .arg("eng")
        .arg("--psm")
        .arg("6") // Assume single uniform block of text
        .arg("tsv") // Output TSV format
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Tesseract failed: {}", stderr));
    }

    // Read TSV output
    let tsv_path = format!("{}.tsv", output_base);
    let tsv_content = std::fs::read_to_string(&tsv_path)
        .map_err(|e| anyhow!("Failed to read Tesseract output: {}", e))?;

    // Clean up output file
    let _ = std::fs::remove_file(&tsv_path);

    // Parse TSV output
    parse_tsv_output(&tsv_content)
}

/// Parses Tesseract TSV output into structured OcrLine data
fn parse_tsv_output(tsv: &str) -> Result<Vec<OcrLine>> {
    let mut lines: Vec<OcrLine> = Vec::new();
    let mut current_line_num: i32 = -1;
    let mut current_words: Vec<OcrWord> = Vec::new();
    let mut current_conf_sum: f32 = 0.0;
    let mut current_word_count: usize = 0;

    for line in tsv.lines().skip(1) {
        // Skip header
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 12 {
            continue;
        }

        // TSV fields: level, page_num, block_num, par_num, line_num, word_num,
        //             left, top, width, height, conf, text
        let level: i32 = fields[0].parse().unwrap_or(-1);
        let line_num: i32 = fields[4].parse().unwrap_or(-1);
        let conf: f32 = fields[10].parse().unwrap_or(-1.0);
        let text = fields[11].trim();

        // Level 5 = word
        if level != 5 {
            continue;
        }

        // Skip empty text
        if text.is_empty() {
            continue;
        }

        // Check if we've moved to a new line
        if line_num != current_line_num && current_line_num >= 0 {
            // Save previous line
            if !current_words.is_empty() {
                let avg_conf = if current_word_count > 0 {
                    current_conf_sum / current_word_count as f32
                } else {
                    0.0
                };
                let line_text = current_words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(OcrLine {
                    text: line_text,
                    words: current_words,
                    confidence: avg_conf,
                });
            }
            current_words = Vec::new();
            current_conf_sum = 0.0;
            current_word_count = 0;
        }

        current_line_num = line_num;

        // Add word
        if conf >= 0.0 {
            current_words.push(OcrWord {
                text: text.to_string(),
                confidence: conf,
            });
            current_conf_sum += conf;
            current_word_count += 1;
        }
    }

    // Don't forget the last line
    if !current_words.is_empty() {
        let avg_conf = if current_word_count > 0 {
            current_conf_sum / current_word_count as f32
        } else {
            0.0
        };
        let line_text = current_words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(OcrLine {
            text: line_text,
            words: current_words,
            confidence: avg_conf,
        });
    }

    Ok(lines)
}

/// Simple OCR that just returns raw text (for debugging)
pub fn recognize_image_simple(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<String> {
    let tesseract_exe = find_tesseract_executable()?;
    let tessdata_dir = find_tessdata_dir()?;

    // Save image to temporary file
    let temp_input = NamedTempFile::with_suffix(".png")?;
    img.save(temp_input.path())?;

    // Run Tesseract to stdout
    let output = Command::new(&tesseract_exe)
        .arg(temp_input.path())
        .arg("stdout")
        .arg("--tessdata-dir")
        .arg(&tessdata_dir)
        .arg("-l")
        .arg("eng")
        .arg("--psm")
        .arg("6")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Tesseract failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
