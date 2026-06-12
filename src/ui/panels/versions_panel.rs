use super::super::components::{format_size, open_in_explorer};
use crate::theme::AppTheme;
use crate::version_manager::{self, SortOrder, VersionManager};
use eframe::egui;
use std::path::PathBuf;

pub fn render(
    ui: &mut egui::Ui,
    version_mgr: &mut VersionManager,
    pending_delete: &mut Option<PathBuf>,
    theme: &AppTheme,
) {
    ui.add_space(12.0);
    ui.add(egui::Separator::default());
    ui.add_space(12.0);

    ui.label(
        egui::RichText::new("Versiones guardadas:")
            .heading()
            .color(theme.text_primary),
    );
    ui.add_space(8.0);

    // Sort & filter controls
    if !version_mgr.versions.is_empty() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Orden:")
                    .small()
                    .color(theme.text_secondary),
            );
            egui::ComboBox::from_id_salt("sort_order")
                .selected_text(match version_mgr.sort_order {
                    SortOrder::Newest => "Más recientes",
                    SortOrder::Oldest => "Más antiguas",
                    SortOrder::Largest => "Más grandes",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut version_mgr.sort_order,
                        SortOrder::Newest,
                        "Más recientes",
                    );
                    ui.selectable_value(
                        &mut version_mgr.sort_order,
                        SortOrder::Oldest,
                        "Más antiguas",
                    );
                    ui.selectable_value(
                        &mut version_mgr.sort_order,
                        SortOrder::Largest,
                        "Más grandes",
                    );
                });

            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Filtrar:")
                    .small()
                    .color(theme.text_secondary),
            );
            let mut filter = version_mgr.filter.clone();
            let filter_resp = ui.add(
                egui::TextEdit::singleline(&mut filter)
                    .desired_width(120.0)
                    .hint_text("buscar..."),
            );
            if filter_resp.changed() {
                version_mgr.filter = filter;
                version_mgr.refresh();
            } else if filter != version_mgr.filter {
                version_mgr.refresh();
            }
        });
        ui.add_space(4.0);
    }

    if version_mgr.versions.is_empty() {
        ui.label(
            egui::RichText::new("No hay versiones guardadas.")
                .italics()
                .color(theme.text_secondary),
        );
    } else {
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .auto_shrink(false)
            .show(ui, |ui| {
                for version in &version_mgr.versions {
                    ui.horizontal(|ui| {
                        if let Some(name) = version.file_name() {
                            let size = version_manager::folder_size(version);
                            ui.label(format!(
                                "{} ({})",
                                name.to_string_lossy(),
                                format_size(size)
                            ));
                        }

                        let open_btn = egui::Button::new("Abrir")
                            .fill(theme.btn_bg)
                            .stroke(egui::Stroke::new(1.0, theme.border_color))
                            .rounding(4.0);
                        if ui
                            .add(open_btn)
                            .on_hover_text("Abrir esta versión en el explorador de archivos")
                            .clicked()
                        {
                            open_in_explorer(version);
                        }

                        let delete_btn = egui::Button::new("🗑")
                            .fill(theme.danger_bg)
                            .stroke(egui::Stroke::new(1.0, theme.danger_border))
                            .rounding(4.0);
                        if ui
                            .add(delete_btn)
                            .on_hover_text("Eliminar esta versión permanentemente")
                            .clicked()
                        {
                            *pending_delete = Some(version.clone());
                        }
                    });
                }
            });
    }
}
