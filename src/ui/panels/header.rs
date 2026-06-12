use crate::theme::AppTheme;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, logo: &Option<egui::TextureHandle>, _theme: &AppTheme) {
    ui.add_space(12.0);
    if let Some(logo) = logo {
        ui.add(
            egui::Image::from_texture((logo.id(), logo.size_vec2()))
                .max_height(72.0)
                .rounding(6.0),
        );
    }
    ui.add_space(16.0);
}
