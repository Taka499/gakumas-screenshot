//! JSON export for statistics data.

use super::statistics::DataSetStats;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Export statistics to a JSON file.
///
/// The output is pretty-printed for human readability.
pub fn export_to_json(stats: &DataSetStats, output_path: &Path) -> Result<()> {
    let json =
        serde_json::to_string_pretty(stats).context("Failed to serialize statistics to JSON")?;

    let mut file = File::create(output_path)
        .context(format!("Failed to create JSON file: {}", output_path.display()))?;

    file.write_all(json.as_bytes())
        .context("Failed to write JSON data")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::statistics::ColumnStats;
    use tempfile::tempdir;

    #[test]
    fn test_export_to_json() {
        let stats = DataSetStats {
            total_runs: 5,
            columns: vec![ColumnStats {
                stage: 1,
                criterion: 1,
                count: 5,
                mean: 100.0,
                median: 100.0,
                mode: 100,
                min: 90,
                max: 110,
                std_dev: 5.0,
                quartile_1: 95.0,
                quartile_3: 105.0,
            }],
        };

        let dir = tempdir().unwrap();
        let path = dir.path().join("stats.json");

        export_to_json(&stats, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"total_runs\": 5"));
        assert!(content.contains("\"mean\": 100.0"));
        assert!(content.contains("\"stage\": 1"));
    }
}
