//! GUI module for the application.
//!
//! Provides a graphical interface using egui/eframe for user interaction.

pub mod render;
pub mod state;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

use eframe::egui::{self, TextureHandle, Vec2};

use crate::automation::runner::{is_automation_running, start_automation};
use crate::automation::state::{request_abort, ABORT_REQUESTED};

use state::{AutomationStatus, GuiState};

/// Embedded guide images (placeholders - will be replaced with actual screenshots).
const GUIDE_IMAGE_1: &[u8] = include_bytes!("../../resources/guide/step1_contest_mode.png");
const GUIDE_IMAGE_2: &[u8] = include_bytes!("../../resources/guide/step2_rehearsal_page.png");

/// Main GUI application struct.
pub struct GuiApp {
    /// Application state.
    state: GuiState,
    /// Loaded guide image textures.
    guide_images: [Option<TextureHandle>; 2],
    /// Flag to track if images have been loaded.
    images_loaded: bool,
}

impl GuiApp {
    /// Create a new GUI application instance.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure fonts to support Japanese
        Self::setup_fonts(&cc.egui_ctx);

        Self {
            state: GuiState::default(),
            guide_images: [None, None],
            images_loaded: false,
        }
    }

    /// Setup fonts with Japanese support.
    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        // Try to load a Japanese system font
        // Common Japanese fonts on Windows: Yu Gothic, Meiryo, MS Gothic
        let font_paths = [
            "C:\\Windows\\Fonts\\YuGothM.ttc",  // Yu Gothic Medium
            "C:\\Windows\\Fonts\\meiryo.ttc",   // Meiryo
            "C:\\Windows\\Fonts\\msgothic.ttc", // MS Gothic
        ];

        let mut font_loaded = false;
        for font_path in &font_paths {
            if let Ok(font_data) = std::fs::read(font_path) {
                fonts.font_data.insert(
                    "japanese_font".to_owned(),
                    egui::FontData::from_owned(font_data).into(),
                );

                // Add Japanese font as first priority for proportional text
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "japanese_font".to_owned());

                // Also add for monospace
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, "japanese_font".to_owned());

                crate::log(&format!("Loaded Japanese font from: {}", font_path));
                font_loaded = true;
                break;
            }
        }

        if !font_loaded {
            crate::log("Warning: Could not load Japanese font. Text may not display correctly.");
        }

        ctx.set_fonts(fonts);
    }

    /// Load guide images as textures.
    fn load_images(&mut self, ctx: &egui::Context) {
        if self.images_loaded {
            return;
        }

        // Load image 1
        if let Ok(image) = image::load_from_memory(GUIDE_IMAGE_1) {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.into_raw();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            self.guide_images[0] = Some(ctx.load_texture(
                "guide_image_1",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }

        // Load image 2
        if let Ok(image) = image::load_from_memory(GUIDE_IMAGE_2) {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.into_raw();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
            self.guide_images[1] = Some(ctx.load_texture(
                "guide_image_2",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }

        self.images_loaded = true;
    }

    /// Update automation status by polling the automation runner.
    fn update_automation_status(&mut self) {
        let is_running = is_automation_running();
        let is_aborted = ABORT_REQUESTED.load(Ordering::SeqCst);

        match &self.state.status {
            AutomationStatus::Running { total, start_time, .. } => {
                if !is_running {
                    // Automation finished
                    if is_aborted {
                        self.state.status = AutomationStatus::Aborted;
                    } else {
                        // Get the session path from the latest output
                        let session_path = self.state.latest_session_path.clone()
                            .unwrap_or_else(|| crate::paths::get_output_dir());
                        self.state.status = AutomationStatus::Completed {
                            total: *total,
                            session_path,
                        };
                    }
                } else {
                    // Still running - update progress
                    // TODO: Get actual progress from runner
                    let current = crate::automation::runner::get_current_iteration();
                    let state_desc = crate::automation::runner::get_current_state_description();
                    self.state.status = AutomationStatus::Running {
                        current,
                        total: *total,
                        state_description: state_desc,
                        start_time: *start_time,
                    };
                }
            }
            AutomationStatus::Idle | AutomationStatus::Completed { .. } |
            AutomationStatus::Aborted | AutomationStatus::Error(_) => {
                // Check if automation started externally (shouldn't happen with GUI)
                if is_running && !matches!(self.state.status, AutomationStatus::Running { .. }) {
                    self.state.status = AutomationStatus::Running {
                        current: 0,
                        total: self.state.iterations,
                        state_description: "開始中...".to_string(),
                        start_time: Instant::now(),
                    };
                }
            }
        }
    }

    /// Handle start button click.
    fn handle_start(&mut self) {
        let iterations = self.state.iterations;

        // Create session folder with timestamp
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let session_path = crate::paths::get_output_dir().join(&timestamp);

        // Store session path for later
        self.state.latest_session_path = Some(session_path.clone());

        // Start automation
        match start_automation(Some(iterations)) {
            Ok(()) => {
                self.state.status = AutomationStatus::Running {
                    current: 0,
                    total: iterations,
                    state_description: "開始中...".to_string(),
                    start_time: Instant::now(),
                };
                self.state.automation_start_time = Some(Instant::now());
                crate::log(&format!("GUI: Started automation with {} iterations", iterations));
            }
            Err(e) => {
                self.state.status = AutomationStatus::Error(e.to_string());
                crate::log(&format!("GUI: Failed to start automation: {}", e));
            }
        }
    }

    /// Handle stop button click.
    fn handle_stop(&mut self) {
        request_abort();
        crate::log("GUI: Requested automation abort");
    }

    /// Handle generate charts button click.
    fn handle_generate_charts(&self) {
        crate::log("GUI: Generating charts...");
        match crate::analysis::generate_analysis() {
            Ok((chart_paths, json_path)) => {
                crate::log(&format!(
                    "GUI: Charts generated: {} files, stats: {}",
                    chart_paths.len(),
                    json_path.display()
                ));
            }
            Err(e) => {
                crate::log(&format!("GUI: Failed to generate charts: {}", e));
            }
        }
    }

    /// Handle open folder button click.
    fn handle_open_folder(&self) {
        if let Some(path) = &self.state.latest_session_path {
            // Open folder in Windows Explorer
            if let Err(e) = std::process::Command::new("explorer")
                .arg(path)
                .spawn()
            {
                crate::log(&format!("GUI: Failed to open folder: {}", e));
            }
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Load images on first frame
        self.load_images(ctx);

        // Poll automation status
        self.update_automation_status();

        // Request repaint while automation is running (for progress updates)
        if self.state.status.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("学マス リハーサル統計自動化ツール");
            ui.add_space(16.0);

            // Scrollable content
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Instructions section
                render::render_instructions(ui, &self.guide_images);

                // Controls section (iteration input, start/stop buttons)
                let (start_clicked, stop_clicked) = render::render_controls(ui, &mut self.state);

                if start_clicked {
                    self.handle_start();
                }
                if stop_clicked {
                    self.handle_stop();
                }

                // Progress section
                render::render_progress(ui, &self.state);

                // Action buttons section
                let (generate_clicked, open_folder_clicked) = render::render_actions(ui, &self.state);

                if generate_clicked {
                    self.handle_generate_charts();
                }
                if open_folder_clicked {
                    self.handle_open_folder();
                }
            });
        });
    }
}

/// Run the GUI application.
/// This function blocks until the window is closed.
pub fn run_gui() -> eframe::Result<()> {
    crate::log("GUI: Creating native options...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(600.0, 500.0))
            .with_min_inner_size(Vec2::new(400.0, 400.0))
            .with_title("Gakumas Rehearsal Automation")
            // Disable drag-and-drop to avoid COM conflict with RoInitialize (multithreaded)
            .with_drag_and_drop(false),
        ..Default::default()
    };

    crate::log("GUI: Calling eframe::run_native...");

    eframe::run_native(
        "Gakumas Rehearsal Automation",
        options,
        Box::new(|cc| {
            crate::log("GUI: Creating GuiApp instance...");
            Ok(Box::new(GuiApp::new(cc)))
        }),
    )
}
