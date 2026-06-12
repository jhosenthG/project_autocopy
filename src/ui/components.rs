use eframe::egui;
use std::path::PathBuf;

use crate::theme::AppTheme;

/// Format bytes to a human-readable string (KB / MB / GB).
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// Open a folder in Windows Explorer.
pub fn open_in_explorer(path: &PathBuf) {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .ok();
    }
}

/// Draw a path selector row for source or destination.
///
/// Returns `true` when a folder was picked via the file dialog (the caller
/// may want to refresh the version list in that case).
pub fn path_row(
    ui: &mut egui::Ui,
    label: &str,
    path: &mut Option<PathBuf>,
    valid: Option<bool>,
    theme: &AppTheme,
) -> bool {
    let (browse_tt, open_tt, field_tt) = if label == "Origen:" {
        (
            "Seleccionar carpeta origen",
            "Abrir carpeta origen en el explorador",
            "Ruta de la carpeta que deseas respaldar",
        )
    } else {
        (
            "Seleccionar carpeta destino",
            "Abrir carpeta destino en el explorador",
            "Ruta donde se guardarán los respaldos",
        )
    };

    let mut picked = false;
    ui.label(egui::RichText::new(label).color(theme.text_secondary));

    ui.horizontal(|ui| {
        let current_text = path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let hint = if current_text.is_empty() {
            "Suelta una carpeta aquí o escribe la ruta"
        } else {
            ""
        };

        let mut text = current_text;
        let resp = ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text(hint)
                .desired_width(300.0),
        );
        resp.on_hover_text(field_tt);

        // Update path from text — allow clearing by setting to None
        if text.is_empty() {
            *path = None;
        } else {
            *path = Some(PathBuf::from(text));
        }

        // Visual validation indicator
        match valid {
            Some(true) => {
                ui.colored_label(theme.success_color, "\u{2713}");
            }
            Some(false) => {
                ui.colored_label(egui::Color32::RED, "\u{2717}");
            }
            None => {}
        }

        // Browse button
        let browse_btn = egui::Button::new("Examinar...")
            .fill(theme.btn_bg)
            .stroke(egui::Stroke::new(1.0, theme.border_color))
            .rounding(4.0);
        if ui.add(browse_btn).on_hover_text(browse_tt).clicked() {
            if let Some(picked_path) = rfd::FileDialog::new().pick_folder() {
                *path = Some(picked_path);
                picked = true;
            }
        }

        // Open-in-explorer button (only for existing directories)
        if let Some(ref p) = path.clone() {
            if p.exists() {
                let open_btn = egui::Button::new("\u{1F4C2}")
                    .fill(theme.btn_bg)
                    .stroke(egui::Stroke::new(1.0, theme.border_color))
                    .rounding(4.0);
                if ui.add(open_btn).on_hover_text(open_tt).clicked() {
                    open_in_explorer(p);
                }
            }
        }
    });
    picked
}
