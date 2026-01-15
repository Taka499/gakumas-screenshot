//! GUI rendering functions.
//!
//! Contains UI layout and component rendering logic.

use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};

use super::state::{AutomationStatus, GuiState};

/// Render the instruction panel with two guide images.
pub fn render_instructions(
    ui: &mut egui::Ui,
    guide_images: &[Option<TextureHandle>; 2],
) {
    ui.heading("使い方");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        // Calculate available width for two images side by side
        let available_width = ui.available_width();
        let image_width = (available_width - 20.0) / 2.0; // 20px gap between images
        let image_height = image_width * 0.6; // 16:10 aspect ratio

        // Image 1: Contest mode navigation
        ui.vertical(|ui| {
            ui.set_width(image_width);

            if let Some(texture) = &guide_images[0] {
                ui.image((texture.id(), Vec2::new(image_width, image_height)));
            } else {
                // Placeholder when image not loaded
                let (rect, _response) = ui.allocate_exact_size(
                    Vec2::new(image_width, image_height),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 4.0, Color32::from_gray(200));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "画像1",
                    egui::FontId::proportional(16.0),
                    Color32::from_gray(100),
                );
            }

            ui.add_space(4.0);
            ui.label(RichText::new("① コンテストモードへ").strong());
        });

        ui.add_space(20.0);

        // Image 2: Rehearsal preparation page
        ui.vertical(|ui| {
            ui.set_width(image_width);

            if let Some(texture) = &guide_images[1] {
                ui.image((texture.id(), Vec2::new(image_width, image_height)));
            } else {
                // Placeholder when image not loaded
                let (rect, _response) = ui.allocate_exact_size(
                    Vec2::new(image_width, image_height),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 4.0, Color32::from_gray(200));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "画像2",
                    egui::FontId::proportional(16.0),
                    Color32::from_gray(100),
                );
            }

            ui.add_space(4.0);
            ui.label(RichText::new("② この画面で待機").strong());
        });
    });
}

/// Render the iteration input and control buttons.
/// Returns (start_clicked, stop_clicked).
pub fn render_controls(
    ui: &mut egui::Ui,
    state: &mut GuiState,
) -> (bool, bool) {
    let mut start_clicked = false;
    let mut stop_clicked = false;

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Iteration count input
    ui.horizontal(|ui| {
        ui.label("実行回数:");
        ui.add(
            egui::DragValue::new(&mut state.iterations)
                .range(1..=9999)
                .speed(1.0)
        );
        ui.label("回");
    });

    ui.add_space(16.0);

    // Start/Stop buttons
    ui.horizontal(|ui| {
        let is_running = state.status.is_running();

        // Start button - disabled while running
        ui.add_enabled_ui(!is_running, |ui| {
            if ui.button(RichText::new("▶ 開始").size(16.0)).clicked() {
                start_clicked = true;
            }
        });

        ui.add_space(20.0);

        // Stop button - enabled only while running
        ui.add_enabled_ui(is_running, |ui| {
            if ui.button(RichText::new("◼ 停止").size(16.0)).clicked() {
                stop_clicked = true;
            }
        });
    });

    (start_clicked, stop_clicked)
}

/// Render the progress display section.
pub fn render_progress(
    ui: &mut egui::Ui,
    state: &GuiState,
) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Status text
    ui.horizontal(|ui| {
        ui.label("状態:");

        let status_color = match &state.status {
            AutomationStatus::Idle => Color32::GRAY,
            AutomationStatus::Running { .. } => Color32::from_rgb(0, 120, 200),
            AutomationStatus::Completed { .. } => Color32::from_rgb(0, 150, 0),
            AutomationStatus::Aborted => Color32::from_rgb(200, 150, 0),
            AutomationStatus::Error(_) => Color32::from_rgb(200, 0, 0),
        };

        ui.label(RichText::new(state.status.status_text()).color(status_color));
    });

    // Progress bar
    ui.add_space(8.0);
    let progress = state.status.progress();

    ui.horizontal(|ui| {
        ui.label("進捗:");

        let progress_bar = egui::ProgressBar::new(progress)
            .show_percentage()
            .animate(state.status.is_running());

        ui.add_sized([ui.available_width() - 100.0, 20.0], progress_bar);
    });

    // Elapsed time (if running)
    if let Some(elapsed) = state.status.elapsed_text() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("経過時間:");
            ui.label(elapsed);
        });
    }
}

/// Render the action buttons (Generate Charts, Open Folder).
/// Returns (generate_charts_clicked, open_folder_clicked).
pub fn render_actions(
    ui: &mut egui::Ui,
    state: &GuiState,
) -> (bool, bool) {
    let mut generate_clicked = false;
    let mut open_folder_clicked = false;

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        // Generate Charts button
        if ui.button("📊 グラフを生成").clicked() {
            generate_clicked = true;
        }

        ui.add_space(20.0);

        // Open Folder button - enabled only if we have a session path
        ui.add_enabled_ui(state.latest_session_path.is_some(), |ui| {
            if ui.button("📁 フォルダを開く").clicked() {
                open_folder_clicked = true;
            }
        });
    });

    (generate_clicked, open_folder_clicked)
}
