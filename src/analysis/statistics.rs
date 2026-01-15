//! Statistics calculation for score data.
//!
//! Calculates mean, median, mode, min, max, standard deviation, and quartiles.

use super::csv_reader::DataSet;
use serde::Serialize;
use std::collections::HashMap;

/// Statistics for one score column (one stage/criterion combination).
#[derive(Debug, Clone, Serialize)]
pub struct ColumnStats {
    /// Stage number (1, 2, or 3)
    pub stage: usize,
    /// Criterion number (1, 2, or 3)
    pub criterion: usize,
    /// Number of values
    pub count: usize,
    /// Arithmetic mean (average)
    pub mean: f64,
    /// Median (middle value)
    pub median: f64,
    /// Mode (most frequent value)
    pub mode: u32,
    /// Minimum value
    pub min: u32,
    /// Maximum value
    pub max: u32,
    /// Standard deviation (population)
    pub std_dev: f64,
    /// First quartile (25th percentile)
    pub quartile_1: f64,
    /// Third quartile (75th percentile)
    pub quartile_3: f64,
}

/// Statistics for the entire dataset.
#[derive(Debug, Clone, Serialize)]
pub struct DataSetStats {
    /// Total number of runs
    pub total_runs: usize,
    /// Statistics for each of the 9 columns
    pub columns: Vec<ColumnStats>,
}

impl DataSetStats {
    /// Calculate statistics for all columns in the dataset.
    pub fn from_dataset(data: &DataSet) -> Self {
        let mut columns = Vec::with_capacity(9);

        for stage in 0..3 {
            for criterion in 0..3 {
                let values = data.column_values(stage, criterion);
                let stats = calculate_column_stats(&values, stage + 1, criterion + 1);
                columns.push(stats);
            }
        }

        DataSetStats {
            total_runs: data.len(),
            columns,
        }
    }
}

/// Calculate statistics for a single column of values.
fn calculate_column_stats(values: &[u32], stage: usize, criterion: usize) -> ColumnStats {
    if values.is_empty() {
        return ColumnStats {
            stage,
            criterion,
            count: 0,
            mean: 0.0,
            median: 0.0,
            mode: 0,
            min: 0,
            max: 0,
            std_dev: 0.0,
            quartile_1: 0.0,
            quartile_3: 0.0,
        };
    }

    let count = values.len();

    // Sort for median, quartiles, min, max
    let mut sorted: Vec<u32> = values.to_vec();
    sorted.sort();

    let min = sorted[0];
    let max = sorted[count - 1];

    // Mean
    let sum: u64 = values.iter().map(|&v| v as u64).sum();
    let mean = sum as f64 / count as f64;

    // Median
    let median = calculate_median(&sorted);

    // Quartiles
    let quartile_1 = calculate_percentile(&sorted, 25.0);
    let quartile_3 = calculate_percentile(&sorted, 75.0);

    // Mode
    let mode = calculate_mode(values);

    // Standard deviation (population formula)
    let variance: f64 = values
        .iter()
        .map(|&v| {
            let diff = v as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / count as f64;
    let std_dev = variance.sqrt();

    ColumnStats {
        stage,
        criterion,
        count,
        mean,
        median,
        mode,
        min,
        max,
        std_dev,
        quartile_1,
        quartile_3,
    }
}

/// Calculate median from sorted values.
fn calculate_median(sorted: &[u32]) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n % 2 == 0 {
        // Even: average of two middle values
        let mid = n / 2;
        (sorted[mid - 1] as f64 + sorted[mid] as f64) / 2.0
    } else {
        // Odd: middle value
        sorted[n / 2] as f64
    }
}

/// Calculate percentile using linear interpolation.
fn calculate_percentile(sorted: &[u32], percentile: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return sorted[0] as f64;
    }

    // Index in range [0, n-1]
    let index = (percentile / 100.0) * (n - 1) as f64;
    let lower_idx = index.floor() as usize;
    let upper_idx = index.ceil() as usize;

    if lower_idx == upper_idx {
        sorted[lower_idx] as f64
    } else {
        let frac = index.fract();
        let lower = sorted[lower_idx] as f64;
        let upper = sorted[upper_idx] as f64;
        lower + (upper - lower) * frac
    }
}

/// Calculate mode (most frequent value).
/// If there's a tie, returns the smallest value.
fn calculate_mode(values: &[u32]) -> u32 {
    if values.is_empty() {
        return 0;
    }

    let mut counts: HashMap<u32, usize> = HashMap::new();
    for &v in values {
        *counts.entry(v).or_insert(0) += 1;
    }

    // Find max count
    let max_count = counts.values().max().copied().unwrap_or(0);

    // Find smallest value with max count
    counts
        .into_iter()
        .filter(|&(_, count)| count == max_count)
        .map(|(value, _)| value)
        .min()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        let values = vec![1, 2, 3, 4, 5];
        let stats = calculate_column_stats(&values, 1, 1);
        assert!((stats.mean - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_median_odd() {
        let values = vec![1, 2, 3, 4, 5];
        let stats = calculate_column_stats(&values, 1, 1);
        assert!((stats.median - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_median_even() {
        let values = vec![1, 2, 3, 4];
        let stats = calculate_column_stats(&values, 1, 1);
        assert!((stats.median - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_min_max() {
        let values = vec![5, 1, 3, 9, 2];
        let stats = calculate_column_stats(&values, 1, 1);
        assert_eq!(stats.min, 1);
        assert_eq!(stats.max, 9);
    }

    #[test]
    fn test_mode() {
        let values = vec![1, 2, 2, 3, 3, 3, 4];
        let stats = calculate_column_stats(&values, 1, 1);
        assert_eq!(stats.mode, 3);
    }

    #[test]
    fn test_mode_tie() {
        // 1 appears twice, 2 appears twice - should return smaller (1)
        let values = vec![1, 1, 2, 2, 3];
        let stats = calculate_column_stats(&values, 1, 1);
        assert_eq!(stats.mode, 1);
    }

    #[test]
    fn test_std_dev() {
        // Values: 1, 2, 3, 4, 5
        // Mean: 3
        // Variance: ((1-3)^2 + (2-3)^2 + (3-3)^2 + (4-3)^2 + (5-3)^2) / 5 = (4+1+0+1+4)/5 = 2
        // Std dev: sqrt(2) ≈ 1.414
        let values = vec![1, 2, 3, 4, 5];
        let stats = calculate_column_stats(&values, 1, 1);
        assert!((stats.std_dev - 1.414).abs() < 0.01);
    }

    #[test]
    fn test_quartiles() {
        // For [1, 2, 3, 4, 5]:
        // Q1 (25th percentile): index = 0.25 * 4 = 1.0 -> value at index 1 = 2
        // Q3 (75th percentile): index = 0.75 * 4 = 3.0 -> value at index 3 = 4
        let values = vec![1, 2, 3, 4, 5];
        let stats = calculate_column_stats(&values, 1, 1);
        assert!((stats.quartile_1 - 2.0).abs() < 0.001);
        assert!((stats.quartile_3 - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_single_value() {
        let values = vec![42];
        let stats = calculate_column_stats(&values, 1, 1);
        assert!((stats.mean - 42.0).abs() < 0.001);
        assert!((stats.median - 42.0).abs() < 0.001);
        assert_eq!(stats.min, 42);
        assert_eq!(stats.max, 42);
        assert_eq!(stats.mode, 42);
        assert!((stats.std_dev - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_empty_values() {
        let values: Vec<u32> = vec![];
        let stats = calculate_column_stats(&values, 1, 1);
        assert_eq!(stats.count, 0);
        assert!((stats.mean - 0.0).abs() < 0.001);
    }
}
