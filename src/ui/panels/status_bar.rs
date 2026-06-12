use crate::theme::AppTheme;
use eframe::egui;

pub fn render(
    ui: &mut egui::Ui,
    backup_active: bool,
    schedule_enabled: bool,
    last_backup_time: &Option<String>,
    next_backup_display: &Option<String>,
    theme: &AppTheme,
) {
    ui.add_space(16.0);
    ui.add(egui::Separator::default());
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Estado:")
                .small()
                .color(theme.text_secondary),
        );

        let status = if backup_active {
            "Respaldando...".to_string()
        } else if schedule_enabled {
            if let Some(next) = next_backup_display {
                format!("Programado: {}", next)
            } else {
                "Programado: —".to_string()
            }
        } else {
            "Inactivo".to_string()
        };
        ui.colored_label(theme.text_secondary, status);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(last) = last_backup_time {
                ui.label(
                    egui::RichText::new(format!("Último: {}", last))
                        .small()
                        .color(theme.text_secondary),
                );
            }

            ui.label(
                egui::RichText::new("Desarrollado por JhosenthG")
                    .small()
                    .color(theme.muted_color),
            );
        });
    });
}
