//! GUI rendering functions.
//!
//! Contains UI layout and component rendering logic.

use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};

use super::state::{AutomationStatus, GuiState};

/// Render a single guide image with label above.
pub fn render_guide_image(
    ui: &mut egui::Ui,
    texture: &Option<TextureHandle>,
    label: &str,
) {
    // Label above the image
    ui.label(RichText::new(label).strong());
    ui.add_space(4.0);

    let available_width = ui.available_width() - 8.0; // Leave some margin

    if let Some(tex) = texture {
        // Preserve original aspect ratio
        let orig_size = tex.size_vec2();
        let aspect_ratio = orig_size.y / orig_size.x;
        let image_height = available_width * aspect_ratio;
        ui.image((tex.id(), Vec2::new(available_width, image_height)));
    } else {
        // Placeholder when image not loaded (use 16:9 as default)
        let image_height = available_width * 1.78; // 9:16 portrait ratio
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(available_width, image_height),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 4.0, Color32::from_gray(200));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "画像",
            egui::FontId::proportional(16.0),
            Color32::from_gray(100),
        );
    }
}

/// Render the iteration input and control buttons.
/// Returns (start_clicked, stop_clicked).
pub fn render_controls(
    ui: &mut egui::Ui,
    state: &mut GuiState,
) -> (bool, bool) {
    let mut start_clicked = false;
    let mut stop_clicked = false;

    ui.heading("設定");
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

    ui.add_space(12.0);

    // Start/Stop buttons
    ui.horizontal(|ui| {
        let is_running = state.status.is_running();

        // Start button - disabled while running
        ui.add_enabled_ui(!is_running, |ui| {
            if ui.button(RichText::new("▶ 開始").size(16.0)).clicked() {
                start_clicked = true;
            }
        });

        ui.add_space(16.0);

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
    ui.add_space(16.0);
    ui.heading("進捗");
    ui.add_space(8.0);

    // Status text
    let status_color = match &state.status {
        AutomationStatus::Idle => Color32::GRAY,
        AutomationStatus::Running { .. } => Color32::from_rgb(0, 120, 200),
        AutomationStatus::Completed { .. } => Color32::from_rgb(0, 150, 0),
        AutomationStatus::Aborted => Color32::from_rgb(200, 150, 0),
        AutomationStatus::Error(_) => Color32::from_rgb(200, 0, 0),
    };

    ui.label(RichText::new(state.status.status_text()).color(status_color));

    // Progress bar
    ui.add_space(8.0);
    let progress = state.status.progress();

    let progress_bar = egui::ProgressBar::new(progress)
        .show_percentage()
        .animate(state.status.is_running());

    ui.add(progress_bar);

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

    ui.add_space(16.0);
    ui.heading("アクション");
    ui.add_space(8.0);

    // Generate Charts button
    if ui.button("📊 グラフを生成").clicked() {
        generate_clicked = true;
    }

    ui.add_space(8.0);

    // Open Folder button - enabled only if we have a session path
    ui.add_enabled_ui(state.latest_session_path.is_some(), |ui| {
        if ui.button("📁 フォルダを開く").clicked() {
            open_folder_clicked = true;
        }
    });

    (generate_clicked, open_folder_clicked)
}
