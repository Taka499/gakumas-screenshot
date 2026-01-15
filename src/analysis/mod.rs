//! Statistics calculation and chart visualization.
//!
//! This module provides:
//! - CSV reading for automation results
//! - Statistics calculation (mean, median, mode, std_dev, quartiles)
//! - Per-character charts with box plot, histogram, and statistics table
//! - JSON export of statistics

pub mod charts;
pub mod csv_reader;
pub mod export;
pub mod statistics;

pub use csv_reader::DataSet;
pub use statistics::DataSetStats;

use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Runs the full analysis pipeline: read CSV, calculate stats, generate charts, export JSON.
///
/// Returns (chart_paths, json_path) where chart_paths contains 9 PNG files (one per column).
pub fn generate_analysis() -> Result<(Vec<PathBuf>, PathBuf)> {
    let exe_dir = crate::paths::get_exe_dir();
    let csv_path = exe_dir.join("results.csv");
    let json_path = exe_dir.join("statistics.json");

    // Load data
    let data = csv_reader::DataSet::from_csv(&csv_path)?;
    if data.is_empty() {
        return Err(anyhow!("No data in CSV file"));
    }

    crate::log(&format!("Loaded {} runs from CSV", data.len()));

    // Calculate statistics
    let stats = statistics::DataSetStats::from_dataset(&data);

    // Generate per-column charts (9 charts total)
    let chart_paths = charts::generate_all_charts(&data, &stats, &exe_dir)?;
    crate::log(&format!("Generated {} charts", chart_paths.len()));

    // Export JSON
    export::export_to_json(&stats, &json_path)?;
    crate::log(&format!("Statistics JSON saved: {}", json_path.display()));

    Ok((chart_paths, json_path))
}
