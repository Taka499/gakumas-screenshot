//! Chart configuration loaded from chart_config.json.
//!
//! If the config file doesn't exist, default values are used.
//! The config file is read fresh each time charts are generated,
//! so changes take effect without rebuilding.

use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Chart configuration with all customizable values.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ChartConfig {
    /// Font sizes
    pub font: FontConfig,
    /// Colors (RGB values)
    pub colors: ColorConfig,
    /// Layout dimensions
    pub layout: LayoutConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Title font size
    pub title_size: u32,
    /// Table header font size
    pub table_header_size: u32,
    /// Table value font size
    pub table_value_size: u32,
    /// Chart axis label font size
    pub axis_label_size: u32,
    /// Legend font size
    pub legend_size: u32,
    /// Box plot caption font size
    pub box_plot_caption_size: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    /// Primary orange color [R, G, B]
    pub orange_primary: [u8; 3],
    /// Header orange color [R, G, B]
    pub orange_header: [u8; 3],
    /// Light gray background [R, G, B]
    pub light_gray_bg: [u8; 3],
    /// Grid line color [R, G, B]
    pub grid_color: [u8; 3],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// Chart image width
    pub chart_width: u32,
    /// Chart image height
    pub chart_height: u32,
    /// Title area height
    pub title_height: u32,
    /// Table area height
    pub table_height: u32,
    /// Table header row height
    pub table_header_height: i32,
    /// Box plot area width
    pub box_plot_width: u32,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            colors: ColorConfig::default(),
            layout: LayoutConfig::default(),
        }
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            title_size: 32,
            table_header_size: 32,
            table_value_size: 32,
            axis_label_size: 14,
            legend_size: 14,
            box_plot_caption_size: 16,
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            orange_primary: [243, 156, 18],    // #F39C12
            orange_header: [230, 126, 34],     // #E67E22
            light_gray_bg: [245, 245, 245],
            grid_color: [220, 220, 220],
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            chart_width: 900,
            chart_height: 700,
            title_height: 50,
            table_height: 90,
            table_header_height: 40,
            box_plot_width: 300,
        }
    }
}

impl ChartConfig {
    /// Load config from file, or return defaults if file doesn't exist.
    pub fn load(config_path: &Path) -> Self {
        if config_path.exists() {
            match fs::read_to_string(config_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        crate::log(&format!("Loaded chart config from {}", config_path.display()));
                        return config;
                    }
                    Err(e) => {
                        crate::log(&format!(
                            "Failed to parse chart config: {}. Using defaults.",
                            e
                        ));
                    }
                },
                Err(e) => {
                    crate::log(&format!(
                        "Failed to read chart config: {}. Using defaults.",
                        e
                    ));
                }
            }
        }
        Self::default()
    }

    /// Save default config to file (for reference).
    pub fn save_default(config_path: &Path) -> std::io::Result<()> {
        let default_config = Self::default();
        let json = serde_json::to_string_pretty(&default_config).unwrap();
        fs::write(config_path, json)
    }
}

// Implement Serialize for saving defaults
impl serde::Serialize for ChartConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ChartConfig", 3)?;
        state.serialize_field("font", &self.font)?;
        state.serialize_field("colors", &self.colors)?;
        state.serialize_field("layout", &self.layout)?;
        state.end()
    }
}

impl serde::Serialize for FontConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("FontConfig", 6)?;
        state.serialize_field("title_size", &self.title_size)?;
        state.serialize_field("table_header_size", &self.table_header_size)?;
        state.serialize_field("table_value_size", &self.table_value_size)?;
        state.serialize_field("axis_label_size", &self.axis_label_size)?;
        state.serialize_field("legend_size", &self.legend_size)?;
        state.serialize_field("box_plot_caption_size", &self.box_plot_caption_size)?;
        state.end()
    }
}

impl serde::Serialize for ColorConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ColorConfig", 4)?;
        state.serialize_field("orange_primary", &self.orange_primary)?;
        state.serialize_field("orange_header", &self.orange_header)?;
        state.serialize_field("light_gray_bg", &self.light_gray_bg)?;
        state.serialize_field("grid_color", &self.grid_color)?;
        state.end()
    }
}

impl serde::Serialize for LayoutConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LayoutConfig", 6)?;
        state.serialize_field("chart_width", &self.chart_width)?;
        state.serialize_field("chart_height", &self.chart_height)?;
        state.serialize_field("title_height", &self.title_height)?;
        state.serialize_field("table_height", &self.table_height)?;
        state.serialize_field("table_header_height", &self.table_header_height)?;
        state.serialize_field("box_plot_width", &self.box_plot_width)?;
        state.end()
    }
}
