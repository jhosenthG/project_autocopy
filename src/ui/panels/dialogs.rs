use crate::backup_runner::BackupRunner;
use crate::version_manager::VersionManager;
use eframe::egui;
use std::path::PathBuf;

pub fn render_cancel_dialog(
    ctx: &egui::Context,
    show_cancel_dialog: &mut bool,
    runner: &mut BackupRunner,
    ui_state_scheduling_active: &mut bool,
    ui_state_backup_active: &mut bool,
) {
    if !*show_cancel_dialog {
        return;
    }

    egui::Window::new("Cancelar respaldo")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("¿Estás seguro de que deseas cancelar el respaldo?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Sí, cancelar").clicked() {
                    runner.cancel();
                    *ui_state_backup_active = false;
                    *ui_state_scheduling_active = false;
                    *show_cancel_dialog = false;
                }
                if ui.button("Continuar").clicked() {
                    *show_cancel_dialog = false;
                }
            });
        });
}

pub fn render_delete_dialog(
    ctx: &egui::Context,
    pending_delete: &mut Option<PathBuf>,
    version_mgr: &mut VersionManager,
    success_message: &mut Option<String>,
    error_message: &mut Option<String>,
) {
    let path = match pending_delete.clone() {
        Some(p) => p,
        None => return,
    };

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    egui::Window::new("Eliminar versión")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("¿Eliminar permanentemente '{}'?", name));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Sí, eliminar").clicked() {
                    match version_mgr.delete_version(&path) {
                        Ok(()) => {
                            *success_message = Some("Versión eliminada correctamente.".to_string());
                        }
                        Err(_) => {
                            *error_message = Some("Error al eliminar la versión.".to_string());
                        }
                    }
                    *pending_delete = None;
                }
                if ui.button("Cancelar").clicked() {
                    *pending_delete = None;
                }
            });
        });
}
