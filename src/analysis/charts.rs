//! Chart generation using plotters.
//!
//! Generates per-character charts with box plot, distribution histogram, and statistics table.

use super::csv_reader::DataSet;
use super::statistics::ColumnStats;
use anyhow::{Context, Result};
use plotters::prelude::*;
use std::path::Path;

/// Minimum bucket size for histogram distribution.
const MIN_BUCKET_SIZE: u32 = 1000;

/// Calculate bucket size based on data range.
/// Formula: MIN_BUCKET_SIZE * max(floor((max - min) / MIN_BUCKET_SIZE / 100), 1)
fn calculate_bucket_size(min_score: u32, max_score: u32) -> u32 {
    let range = max_score.saturating_sub(min_score);
    let factor = (range / MIN_BUCKET_SIZE / 100).max(1);
    MIN_BUCKET_SIZE * factor
}

/// Build histogram buckets from raw values.
/// Returns (bucket_starts, counts) where bucket_starts[i] is the start of bucket i.
fn build_histogram(values: &[u32], bucket_size: u32) -> (Vec<u32>, Vec<u32>) {
    if values.is_empty() || bucket_size == 0 {
        return (vec![], vec![]);
    }

    let min_val = *values.iter().min().unwrap();
    let max_val = *values.iter().max().unwrap();

    // Align bucket start to bucket_size boundary
    let bucket_start = (min_val / bucket_size) * bucket_size;
    let bucket_end = ((max_val / bucket_size) + 1) * bucket_size;
    let num_buckets = ((bucket_end - bucket_start) / bucket_size) as usize;

    let mut counts = vec![0u32; num_buckets];

    for &val in values {
        let bucket_idx = ((val - bucket_start) / bucket_size) as usize;
        if bucket_idx < counts.len() {
            counts[bucket_idx] += 1;
        }
    }

    let bucket_starts: Vec<u32> = (0..num_buckets)
        .map(|i| bucket_start + (i as u32) * bucket_size)
        .collect();

    (bucket_starts, counts)
}

/// Generate a combined chart for a single column (stage/criterion).
/// Contains: box plot (left), histogram (right), statistics table (bottom).
pub fn generate_column_chart(
    column_name: &str,
    values: &[u32],
    stats: &ColumnStats,
    total_runs: usize,
    output_path: &Path,
) -> Result<()> {
    let root = BitMapBackend::new(output_path, (900, 700)).into_drawing_area();
    root.fill(&WHITE)
        .context("Failed to fill chart background")?;

    // Title
    let title = format!("{} Distribution ({} runs)", column_name, total_runs);
    root.titled(&title, ("sans-serif", 24).into_font())
        .context("Failed to draw title")?;

    // Split into chart area (top) and table area (bottom)
    let (chart_area, table_area) = root.split_vertically(580);

    // Split chart area into box plot (left) and histogram (right)
    let (box_area, hist_area) = chart_area.split_horizontally(350);

    // Draw box plot
    draw_box_plot(&box_area, values, stats)?;

    // Draw histogram
    draw_histogram(&hist_area, values, stats)?;

    // Draw statistics table
    draw_stats_table(&table_area, stats)?;

    root.present().context("Failed to save chart")?;
    Ok(())
}

/// Draw box plot in the given area.
fn draw_box_plot(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    values: &[u32],
    stats: &ColumnStats,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }

    let min_val = stats.min as f64;
    let max_val = stats.max as f64;
    let range = max_val - min_val;
    let y_min = (min_val - range * 0.1).max(0.0);
    let y_max = max_val + range * 0.1;

    let mut chart = ChartBuilder::on(area)
        .caption("Box Plot", ("sans-serif", 18))
        .margin(15)
        .x_label_area_size(30)
        .y_label_area_size(70)
        .build_cartesian_2d(0.0f64..2.0f64, y_min..y_max)
        .context("Failed to build box plot")?;

    chart
        .configure_mesh()
        .disable_x_mesh()
        .disable_x_axis()
        .y_desc("Score")
        .y_label_formatter(&|y| format!("{:.0}", y))
        .draw()
        .context("Failed to draw mesh")?;

    let x_center = 1.0;
    let box_width = 0.4;

    // Colors
    let box_color = RGBColor(70, 130, 180); // Steel blue
    let median_color = RGBColor(220, 50, 50); // Red
    let whisker_color = RGBColor(80, 80, 80); // Dark gray

    // Box fill (Q1 to Q3)
    chart.draw_series(std::iter::once(Rectangle::new(
        [
            (x_center - box_width, stats.quartile_1),
            (x_center + box_width, stats.quartile_3),
        ],
        box_color.mix(0.3).filled(),
    )))?;

    // Box outline
    chart.draw_series(std::iter::once(Rectangle::new(
        [
            (x_center - box_width, stats.quartile_1),
            (x_center + box_width, stats.quartile_3),
        ],
        box_color.stroke_width(2),
    )))?;

    // Median line
    chart.draw_series(std::iter::once(PathElement::new(
        vec![
            (x_center - box_width, stats.median),
            (x_center + box_width, stats.median),
        ],
        median_color.stroke_width(3),
    )))?;

    // Lower whisker
    chart.draw_series(std::iter::once(PathElement::new(
        vec![(x_center, min_val), (x_center, stats.quartile_1)],
        whisker_color.stroke_width(2),
    )))?;

    // Upper whisker
    chart.draw_series(std::iter::once(PathElement::new(
        vec![(x_center, stats.quartile_3), (x_center, max_val)],
        whisker_color.stroke_width(2),
    )))?;

    // Min cap
    let cap_width = 0.25;
    chart.draw_series(std::iter::once(PathElement::new(
        vec![(x_center - cap_width, min_val), (x_center + cap_width, min_val)],
        whisker_color.stroke_width(2),
    )))?;

    // Max cap
    chart.draw_series(std::iter::once(PathElement::new(
        vec![(x_center - cap_width, max_val), (x_center + cap_width, max_val)],
        whisker_color.stroke_width(2),
    )))?;

    Ok(())
}

/// Draw histogram in the given area.
fn draw_histogram(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    values: &[u32],
    stats: &ColumnStats,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }

    let bucket_size = calculate_bucket_size(stats.min, stats.max);
    let (bucket_starts, counts) = build_histogram(values, bucket_size);

    if bucket_starts.is_empty() {
        return Ok(());
    }

    let max_count = *counts.iter().max().unwrap_or(&1);
    let x_min = bucket_starts[0] as f64;
    let x_max = (bucket_starts.last().unwrap() + bucket_size) as f64;

    let mut chart = ChartBuilder::on(area)
        .caption(format!("Distribution (bucket={})", bucket_size), ("sans-serif", 18))
        .margin(15)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(x_min..x_max, 0u32..(max_count + 1))
        .context("Failed to build histogram")?;

    chart
        .configure_mesh()
        .x_desc("Score")
        .y_desc("Count")
        .x_label_formatter(&|x| format!("{:.0}", x))
        .draw()
        .context("Failed to draw mesh")?;

    let bar_color = RGBColor(100, 149, 237); // Cornflower blue

    // Draw bars
    for (i, &start) in bucket_starts.iter().enumerate() {
        let count = counts[i];
        if count > 0 {
            chart.draw_series(std::iter::once(Rectangle::new(
                [
                    (start as f64, 0u32),
                    ((start + bucket_size) as f64, count),
                ],
                bar_color.filled(),
            )))?;
            // Bar outline
            chart.draw_series(std::iter::once(Rectangle::new(
                [
                    (start as f64, 0u32),
                    ((start + bucket_size) as f64, count),
                ],
                BLACK.stroke_width(1),
            )))?;
        }
    }

    Ok(())
}

/// Draw statistics table at the bottom.
fn draw_stats_table(
    area: &DrawingArea<BitMapBackend, plotters::coord::Shift>,
    stats: &ColumnStats,
) -> Result<()> {
    // Table layout
    let table_y = 30;
    let row_height = 25;
    let col_widths = [150, 180, 180, 180, 180]; // Label, MIN, MEAN, MODE, MAX

    let headers = ["", "MIN", "MEAN", "MODE", "MAX"];
    let values = [
        "Value",
        &format!("{}", stats.min),
        &format!("{:.1}", stats.mean),
        &format!("{}", stats.mode),
        &format!("{}", stats.max),
    ];

    let font = ("sans-serif", 16).into_font();
    let header_font = ("sans-serif", 16).into_font().style(FontStyle::Bold);

    // Draw header row
    let mut x_offset = 20;
    for (i, header) in headers.iter().enumerate() {
        area.draw_text(
            header,
            &header_font.color(&BLACK),
            (x_offset, table_y),
        )?;
        x_offset += col_widths[i];
    }

    // Draw separator line
    area.draw(&PathElement::new(
        vec![(20, table_y + row_height - 5), (870, table_y + row_height - 5)],
        BLACK.stroke_width(1),
    ))?;

    // Draw value row
    x_offset = 20;
    for (i, value) in values.iter().enumerate() {
        area.draw_text(
            value,
            &font.color(&BLACK),
            (x_offset, table_y + row_height),
        )?;
        x_offset += col_widths[i];
    }

    Ok(())
}

/// Generate all column charts (9 charts total).
pub fn generate_all_charts(
    data: &DataSet,
    stats: &super::statistics::DataSetStats,
    output_dir: &Path,
) -> Result<Vec<std::path::PathBuf>> {
    let mut paths = Vec::new();

    for stage in 0..3 {
        for criterion in 0..3 {
            let idx = stage * 3 + criterion;
            let col_stats = &stats.columns[idx];
            let values = data.column_values(stage, criterion);
            let column_name = format!("S{}C{}", stage + 1, criterion + 1);
            let filename = format!("chart_{}.png", column_name.to_lowercase());
            let output_path = output_dir.join(&filename);

            generate_column_chart(
                &column_name,
                &values,
                col_stats,
                stats.total_runs,
                &output_path,
            )?;

            paths.push(output_path);
        }
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bucket_size() {
        // Small range: should use MIN_BUCKET_SIZE
        assert_eq!(calculate_bucket_size(0, 50000), 1000);

        // Medium range
        assert_eq!(calculate_bucket_size(0, 150000), 1000);

        // Large range
        assert_eq!(calculate_bucket_size(0, 300000), 3000);
    }

    #[test]
    fn test_build_histogram() {
        let values = vec![1000, 1500, 2000, 2500, 3000, 3500];
        let bucket_size = 1000;
        let (starts, counts) = build_histogram(&values, bucket_size);

        assert_eq!(starts, vec![1000, 2000, 3000]);
        assert_eq!(counts, vec![2, 2, 2]);
    }

    #[test]
    fn test_build_histogram_empty() {
        let values: Vec<u32> = vec![];
        let (starts, counts) = build_histogram(&values, 1000);
        assert!(starts.is_empty());
        assert!(counts.is_empty());
    }
}
