//! Statistics calculation and chart visualization.
//!
//! This module provides:
//! - CSV reading for automation results
//! - Statistics calculation (mean, median, mode, std_dev, quartiles)
//! - Per-character charts with box plot, histogram, and statistics table
//! - JSON export of statistics
//! - Configurable chart styling via chart_config.json

pub mod charts;
pub mod config;
pub mod csv_reader;
pub mod export;
pub mod statistics;

pub use config::ChartConfig;
pub use csv_reader::DataSet;
pub use statistics::DataSetStats;

use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Runs the full analysis pipeline: read CSV, calculate stats, generate charts, export JSON.
///
/// Returns (chart_paths, json_path) where chart_paths contains per-column PNGs plus combined box plot.
pub fn generate_analysis() -> Result<(Vec<PathBuf>, PathBuf)> {
    let exe_dir = crate::paths::get_exe_dir();
    let output_dir = exe_dir.join("output");
    let csv_path = exe_dir.join("results.csv");
    let json_path = output_dir.join("statistics.json");
    let combined_chart_path = output_dir.join("chart_combined.png");
    let config_path = exe_dir.join("chart_config.json");

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
        crate::log(&format!("Created output directory: {}", output_dir.display()));
    }

    // Load chart config (creates default if not exists)
    let config = config::ChartConfig::load(&config_path);

    // Save default config if it doesn't exist (for reference)
    if !config_path.exists() {
        if let Err(e) = config::ChartConfig::save_default(&config_path) {
            crate::log(&format!("Failed to save default chart config: {}", e));
        } else {
            crate::log(&format!(
                "Created default chart_config.json at {}",
                config_path.display()
            ));
        }
    }

    // Load data
    let data = csv_reader::DataSet::from_csv(&csv_path)?;
    if data.is_empty() {
        return Err(anyhow!("No data in CSV file"));
    }

    crate::log(&format!("Loaded {} runs from CSV", data.len()));

    // Calculate statistics
    let stats = statistics::DataSetStats::from_dataset(&data);

    // Generate per-column charts (9 charts total)
    let mut chart_paths = charts::generate_all_charts(&data, &stats, &output_dir, &config)?;
    crate::log(&format!("Generated {} per-column charts", chart_paths.len()));

    // Generate combined box plot
    charts::generate_combined_box_plot(&stats, &combined_chart_path, &config)?;
    chart_paths.push(combined_chart_path.clone());
    crate::log(&format!(
        "Generated combined box plot: {}",
        combined_chart_path.display()
    ));

    // Export JSON
    export::export_to_json(&stats, &json_path)?;
    crate::log(&format!("Statistics JSON saved: {}", json_path.display()));

    Ok((chart_paths, json_path))
}
