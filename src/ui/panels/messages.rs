use crate::theme::AppTheme;
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    error_message: &mut Option<String>,
    success_message: &mut Option<String>,
    theme: &AppTheme,
) {
    if let Some(err) = error_message.as_deref() {
        ui.add_space(8.0);
        ui.colored_label(egui::Color32::RED, err);
        if ui.button("Aceptar").clicked() {
            *error_message = None;
        }
    }

    if let Some(msg) = success_message.as_deref() {
        ui.add_space(8.0);
        ui.colored_label(theme.success_color, msg);
        if ui.button("Aceptar").clicked() {
            *success_message = None;
        }
    }
}
