use crate::backup_runner::BackupRunner;
use crate::theme::AppTheme;
use eframe::egui;

pub fn render(ui: &mut egui::Ui, runner: &BackupRunner, theme: &AppTheme) {
    if runner.progress.started && !runner.progress.finished {
        ui.add_space(12.0);
        ui.add(egui::Separator::default());
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Progreso:").color(theme.text_primary));
        ui.add_space(8.0);

        let fraction = if runner.progress.total_files > 0 {
            (runner.progress.current_index as f32).min(runner.progress.total_files as f32)
                / runner.progress.total_files as f32
        } else {
            0.0
        };

        ui.add(
            egui::ProgressBar::new(fraction)
                .fill(theme.brand_color)
                .rounding(12.0)
                .text(format!(
                    "{}% ({}/{})",
                    (fraction * 100.0) as usize,
                    runner.progress.current_index,
                    runner.progress.total_files
                )),
        );

        if !runner.progress.current_file.is_empty() {
            ui.label(
                egui::RichText::new(format!("Copiando: {}", runner.progress.current_file))
                    .color(theme.text_secondary),
            );
        }

        if let Some((speed_bytes, eta_secs)) = runner.compute_eta(None) {
            let speed_mb = speed_bytes / (1024.0 * 1024.0);
            let eta_min = eta_secs / 60;
            let eta_sec = eta_secs % 60;
            ui.label(
                egui::RichText::new(format!(
                    "Velocidad: ~{} MB/s  |  Tiempo restante: ~{}:{:02} min",
                    speed_mb, eta_min, eta_sec
                ))
                .small()
                .color(theme.text_secondary),
            );
        }
    }
}
