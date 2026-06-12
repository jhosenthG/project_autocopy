use super::super::components::{format_size, path_row};
use crate::copy;
use crate::theme::AppTheme;
use crate::version_manager::VersionManager;
use eframe::egui;
use std::path::PathBuf;

pub fn render(
    ui: &mut egui::Ui,
    source_path: &mut Option<PathBuf>,
    dest_path: &mut Option<PathBuf>,
    source_valid: Option<bool>,
    dest_valid: Option<bool>,
    version_mgr: &mut VersionManager,
    theme: &AppTheme,
) -> bool {
    let _picked_source = path_row(ui, "Origen:", source_path, source_valid, theme);

    ui.add_space(12.0);

    let picked_dest = path_row(ui, "Destino:", dest_path, dest_valid, theme);

    if picked_dest {
        if let Some(dest) = dest_path {
            version_mgr.set_dest(Some(dest.clone()));
            version_mgr.refresh();
        }
    }

    // Available space on destination
    if let Some(dest) = dest_path {
        if dest.exists() {
            if let Ok(avail) = copy::get_available_space(dest) {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!("Espacio disponible: {}", format_size(avail)))
                        .small()
                        .color(theme.text_secondary),
                );
            }
        }
    }

    ui.add_space(12.0);
    ui.add(egui::Separator::default());
    ui.add_space(12.0);

    picked_dest
}
