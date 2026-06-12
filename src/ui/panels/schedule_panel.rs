use crate::config::AppConfig;
use crate::scheduler;
use crate::theme::AppTheme;
use chrono::{DateTime, Local};
use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ScheduleResult {
    pub config_changed: bool,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

impl ScheduleResult {
    fn new() -> Self {
        Self {
            config_changed: false,
            success_message: None,
            error_message: None,
        }
    }
}

pub fn render(
    ui: &mut egui::Ui,
    schedule_enabled: &mut bool,
    schedule_time: &mut String,
    schedule_hour: &mut u32,
    schedule_minute: &mut u32,
    config: &mut AppConfig,
    winsched_active: &mut bool,
    scheduling_active: &mut bool,
    is_backup_active: bool,
    next_backup_display: &Option<String>,
    scheduler_cancel: &Arc<AtomicBool>,
    config_saved_at: &mut Option<DateTime<Local>>,
    theme: &AppTheme,
) -> ScheduleResult {
    let mut result = ScheduleResult::new();

    // --- Header ---
    ui.label(
        egui::RichText::new("Programar respaldo automático")
            .heading()
            .color(theme.text_primary),
    );
    ui.add_space(8.0);

    // --- Checkbox ---
    ui.horizontal(|ui| {
        if ui
            .checkbox(schedule_enabled, "")
            .on_hover_text("Activar respaldo automático diario")
            .clicked()
        {
            if *schedule_enabled {
                *scheduling_active = false;
            } else {
                scheduler_cancel.store(true, Ordering::Relaxed);
            }
            save_schedule_config(config, schedule_enabled, schedule_time);
            *config_saved_at = Some(Local::now());
            result.config_changed = true;
        }
        ui.label(egui::RichText::new("Activar respaldo automático").color(theme.text_primary));
    });
    ui.add_space(8.0);

    // --- Time picker ---
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Hora:").color(theme.text_secondary));
        ui.add_space(8.0);

        let old_h = *schedule_hour;
        let old_m = *schedule_minute;

        ui.add(
            egui::Slider::new(schedule_hour, 0..=23)
                .text("h")
                .clamping(egui::SliderClamping::Always),
        )
        .on_hover_text("Hora del respaldo automático (formato 24h)");

        ui.label(":");

        ui.add(
            egui::Slider::new(schedule_minute, 0..=59)
                .text("m")
                .clamping(egui::SliderClamping::Always),
        )
        .on_hover_text("Minuto del respaldo automático");

        if *schedule_hour != old_h || *schedule_minute != old_m {
            *schedule_time = format!("{:02}:{:02}", schedule_hour, schedule_minute);
            save_schedule_config(config, schedule_enabled, schedule_time);
            *config_saved_at = Some(Local::now());
            *scheduling_active = false;
            result.config_changed = true;
        }
    });
    ui.add_space(8.0);

    // --- Next backup display ---
    if let Some(next) = next_backup_display {
        ui.label(
            egui::RichText::new(format!("Próximo respaldo: {}", next)).color(theme.text_secondary),
        );
    }

    if *scheduling_active && !is_backup_active {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Monitor de horario activo — esperando hora programada")
                .color(theme.text_secondary),
        );
    }

    // --- Windows Task Scheduler ---
    if *schedule_enabled {
        ui.add_space(4.0);
        if *winsched_active {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tarea en Windows: ACTIVA")
                        .small()
                        .color(theme.success_color),
                );
                if ui
                    .add(
                        egui::Button::new("Desactivar tarea de Windows")
                            .fill(theme.danger_bg)
                            .rounding(4.0),
                    )
                    .clicked()
                {
                    match scheduler::unschedule_backup_task() {
                        Ok(()) => {
                            *winsched_active = false;
                            result.success_message =
                                Some("Tarea programada eliminada.".to_string());
                        }
                        Err(_) => {
                            result.error_message =
                                Some("Error al eliminar tarea de Windows.".to_string());
                        }
                    }
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tarea en Windows: INACTIVA")
                        .small()
                        .color(theme.text_secondary),
                );
                if ui
                    .add(
                        egui::Button::new("Crear tarea en Windows")
                            .fill(theme.btn_bg)
                            .rounding(4.0),
                    )
                    .clicked()
                {
                    let exe = std::env::current_exe().unwrap_or_default();
                    match scheduler::schedule_backup_task(&exe, schedule_time) {
                        Ok(()) => {
                            *winsched_active = true;
                            result.success_message =
                                Some("Tarea programada creada en Windows.".to_string());
                        }
                        Err(e) => {
                            result.error_message = Some(format!("Error al crear tarea: {}", e));
                        }
                    }
                }
            });
        }
    }

    result
}

fn save_schedule_config(config: &mut AppConfig, schedule_enabled: &bool, schedule_time: &str) {
    let mut cfg = config.clone();
    cfg.schedule_enabled = *schedule_enabled;
    cfg.schedule_time = Some(schedule_time.to_string());
    let _ = cfg.save();
}
