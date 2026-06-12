use chrono::{Local, Timelike};
use eframe::egui::{self, Widget};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::backup_runner::BackupRunner;
use crate::config::AppConfig;
use crate::copy;
use crate::scheduler;
use crate::theme::AppTheme;
use crate::version_manager::VersionManager;

use super::panels;
use super::state::UiState;

// ---------------------------------------------------------------------------
// Helper: parse "HH:MM" into (hour, minute) or fall back to (14, 30)
// ---------------------------------------------------------------------------
fn parse_schedule_time(time: &Option<String>) -> (u32, u32) {
    time.as_ref()
        .and_then(|t| {
            let parts: Vec<&str> = t.split(':').collect();
            if parts.len() == 2 {
                Some((
                    parts[0].parse::<u32>().unwrap_or(14),
                    parts[1].parse::<u32>().unwrap_or(30),
                ))
            } else {
                None
            }
        })
        .unwrap_or((14, 30))
}

// ===========================================================================
//  Main Application State
// ===========================================================================
pub struct AutoCopyApp {
    // -- Configuration -------------------------------------------------------
    config: AppConfig,
    source_path: Option<PathBuf>,
    dest_path: Option<PathBuf>,
    max_versions: usize,
    schedule_enabled: bool,
    schedule_time: String,
    schedule_hour: u32,
    schedule_minute: u32,

    // -- Services ------------------------------------------------------------
    runner: BackupRunner,
    version_mgr: VersionManager,

    // -- Scheduler control ---------------------------------------------------
    scheduler_cancel: Arc<AtomicBool>,

    // -- UI state (separated) ------------------------------------------------
    ui: UiState,
}

impl AutoCopyApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        let (sched_h, sched_m) = parse_schedule_time(&config.schedule_time);
        let winsched = scheduler::is_scheduled();

        let mut version_mgr = VersionManager::new();
        version_mgr.set_dest(config.last_dest.clone());
        version_mgr.refresh();

        Self {
            config: config.clone(),
            source_path: config.last_source.clone(),
            dest_path: config.last_dest.clone(),
            max_versions: config.max_versions,
            schedule_enabled: config.schedule_enabled,
            schedule_time: config
                .schedule_time
                .clone()
                .unwrap_or_else(|| "14:30".to_string()),
            schedule_hour: sched_h,
            schedule_minute: sched_m,
            runner: BackupRunner::new(),
            version_mgr,
            scheduler_cancel: Arc::new(AtomicBool::new(false)),
            ui: UiState::new(winsched),
        }
    }

    // -- Path validation -----------------------------------------------------

    fn validate_path_fields(&mut self) {
        self.ui.source_valid = match &self.source_path {
            Some(p) if !p.as_os_str().is_empty() => Some(p.exists()),
            _ => None,
        };
        self.ui.dest_valid = match &self.dest_path {
            Some(p) if !p.as_os_str().is_empty() => Some(
                p.exists()
                    && self
                        .source_path
                        .as_ref()
                        .map(|s| s.as_os_str() != p.as_os_str())
                        .unwrap_or(true),
            ),
            _ => None,
        };
    }

    // -- Configuration persistence -------------------------------------------

    fn save_config(&mut self) -> bool {
        let mut config = self.config.clone();
        config.last_source = self.source_path.clone();
        config.last_dest = self.dest_path.clone();
        config.max_versions = self.max_versions;
        config.schedule_enabled = self.schedule_enabled;
        config.schedule_time = Some(self.schedule_time.clone());

        match config.save() {
            Ok(()) => {
                self.ui.config_saved_at = Some(Local::now());
                true
            }
            Err(e) => {
                eprintln!("Failed to save config: {}", e);
                false
            }
        }
    }

    // -- Backup lifecycle ----------------------------------------------------

    fn start_backup(&mut self) {
        let source = match &self.source_path {
            Some(p) => p.clone(),
            None => {
                self.ui.error_message =
                    Some("Por favor selecciona una carpeta origen.".to_string());
                return;
            }
        };
        let dest = match &self.dest_path {
            Some(p) => p.clone(),
            None => {
                self.ui.error_message =
                    Some("Por favor selecciona una carpeta destino.".to_string());
                return;
            }
        };

        if let Err(e) = copy::validate_paths(&source, &dest) {
            self.ui.error_message = Some(format!("Error de validación: {}", e));
            return;
        }

        self.runner.start(source, dest, self.max_versions);
        self.ui.backup_active = true;
        self.ui.scheduling_active = true;
    }

    // -- Scheduling display --------------------------------------------------

    fn update_next_backup_display(&mut self) {
        if self.schedule_enabled {
            let now = Local::now();
            let current_minutes = now.hour() * 60 + now.minute();

            let parts: Vec<&str> = self.schedule_time.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(h), Ok(m)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    let schedule_minutes = h * 60 + m;

                    self.ui.next_backup_display = Some(if schedule_minutes > current_minutes {
                        format!("hoy {:02}:{:02}", h, m)
                    } else {
                        format!("mañana {:02}:{:02}", h, m)
                    });
                    return;
                }
            }
        }
        self.ui.next_backup_display = None;
    }
}

// ===========================================================================
//  eframe::App trait implementation (main update loop)
// ===========================================================================
impl eframe::App for AutoCopyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_next_backup_display();
        self.validate_path_fields();
        self.handle_auto_dismiss_timers();
        self.handle_logo_lazy_load(ctx);
        self.handle_keyboard_shortcuts(ctx);
        self.handle_drag_and_drop(ctx);
        self.update_scheduler();
        self.poll_backup_progress();
        self.render_dialogs(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_ui(ui);
        });
    }
}

// ===========================================================================
//  Update helpers (extracted from update() for clarity)
// ===========================================================================
impl AutoCopyApp {
    fn handle_auto_dismiss_timers(&mut self) {
        if let Some(timer) = self.ui.success_timer {
            if (Local::now() - timer).num_seconds() >= 3 {
                self.ui.success_message = None;
                self.ui.success_timer = None;
            }
        }
        if let Some(saved) = self.ui.config_saved_at {
            if (Local::now() - saved).num_milliseconds() >= 1500 {
                self.ui.config_saved_at = None;
            }
        }
    }

    fn handle_logo_lazy_load(&mut self, ctx: &egui::Context) {
        if self.ui.logo.is_none() {
            let png_bytes = include_bytes!("../../icons/icon_autocopy.png");
            if let Ok(img) = image::load_from_memory(png_bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                self.ui.logo =
                    Some(ctx.load_texture("logo", color_image, egui::TextureOptions::default()));
            }
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let do_backup = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::B));
        if do_backup && self.save_config() {
            self.start_backup();
        }

        let do_save = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S));
        if do_save {
            self.save_config();
        }

        let do_escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if do_escape {
            self.ui.error_message = None;
            self.ui.success_message = None;
            self.ui.show_cancel_dialog = false;
            self.ui.pending_delete = None;
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            for file in &dropped {
                if let Some(path) = &file.path {
                    if path.is_dir() {
                        if self.source_path.is_none() {
                            self.source_path = Some(path.clone());
                        } else {
                            self.dest_path = Some(path.clone());
                            self.version_mgr.set_dest(Some(path.clone()));
                            self.version_mgr.refresh();
                        }
                    }
                }
            }
            ctx.input_mut(|i| i.raw.dropped_files.clear());
        }
    }

    fn update_scheduler(&mut self) {
        if self.schedule_enabled && !self.ui.scheduling_active {
            let schedule_time = self.schedule_time.clone();
            let max_versions = self.max_versions;

            self.ui.scheduling_active = true;
            self.scheduler_cancel = Arc::new(AtomicBool::new(false));
            let cancel = self.scheduler_cancel.clone();

            thread::spawn(move || {
                let mut last_executed_day = String::new();

                loop {
                    thread::sleep(Duration::from_secs(30));

                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }

                    let now = Local::now();
                    let current_time = format!("{:02}:{:02}", now.hour(), now.minute());
                    let current_day = now.format("%Y-%m-%d").to_string();

                    if current_time == schedule_time && current_day != last_executed_day {
                        last_executed_day = current_day.clone();

                        let config = AppConfig::load();
                        let source = match config.last_source {
                            Some(s) => s,
                            None => continue,
                        };
                        let dest = match config.last_dest {
                            Some(d) => d,
                            None => continue,
                        };

                        let (tx, _rx) = mpsc::channel();
                        let cancel_flag = Arc::new(AtomicBool::new(false));

                        thread::spawn(move || {
                            let opts = crate::copy::BackupOptions {
                                cancel_flag,
                                progress_tx: tx,
                            };

                            let result = crate::copy::perform_backup(&source, &dest, opts);

                            if result.is_ok() {
                                let _ = crate::copy::cleanup_old_versions(&dest, max_versions);
                            }
                        });
                    }
                }
            });
        }

        if !self.schedule_enabled {
            self.scheduler_cancel.store(true, Ordering::Relaxed);
            self.ui.scheduling_active = false;
        }
    }

    fn poll_backup_progress(&mut self) {
        if self.runner.poll() {
            self.ui.backup_active = false;
            self.ui.scheduling_active = false;
            self.ui.success_message = Some("Respaldo completado exitosamente.".to_string());
            self.ui.last_backup_time = Some(Local::now().format("%Y-%m-%d %H:%M").to_string());
            if let Some(dest) = &self.dest_path {
                self.version_mgr.set_dest(Some(dest.clone()));
                self.version_mgr.refresh();
            }
        }
    }

    fn render_dialogs(&mut self, ctx: &egui::Context) {
        panels::dialogs::render_cancel_dialog(
            ctx,
            &mut self.ui.show_cancel_dialog,
            &mut self.runner,
            &mut self.ui.scheduling_active,
            &mut self.ui.backup_active,
        );
        panels::dialogs::render_delete_dialog(
            ctx,
            &mut self.ui.pending_delete,
            &mut self.version_mgr,
            &mut self.ui.success_message,
            &mut self.ui.error_message,
        );
    }
}

// ===========================================================================
//  UI Rendering
// ===========================================================================
impl AutoCopyApp {
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        let theme = AppTheme::from_visuals(&ui.style().visuals);

        // -- Logo / Title ----------------------------------------------------
        panels::header::render(ui, &self.ui.logo, &theme);

        // -- Path Configuration ----------------------------------------------
        let _ = panels::path_panel::render(
            ui,
            &mut self.source_path,
            &mut self.dest_path,
            self.ui.source_valid,
            self.ui.dest_valid,
            &mut self.version_mgr,
            &theme,
        );

        // -- Version Settings ------------------------------------------------
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Máximo de versiones:").color(theme.text_secondary));
            ui.add_space(8.0);
            egui::DragValue::new(&mut self.max_versions)
                .range(3..=10)
                .ui(ui)
                .on_hover_text("Número máximo de copias a conservar (3-10)");
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("(mín. 3, máx. 10)")
                    .small()
                    .color(theme.text_secondary),
            );
        });
        ui.add_space(12.0);
        ui.add(egui::Separator::default());
        ui.add_space(12.0);

        // -- Schedule Settings -----------------------------------------------
        let sched_result = panels::schedule_panel::render(
            ui,
            &mut self.schedule_enabled,
            &mut self.schedule_time,
            &mut self.schedule_hour,
            &mut self.schedule_minute,
            &mut self.config,
            &mut self.ui.winsched_active,
            &mut self.ui.scheduling_active,
            self.ui.backup_active,
            &self.ui.next_backup_display,
            &self.scheduler_cancel,
            &mut self.ui.config_saved_at,
            &theme,
        );
        if let Some(msg) = sched_result.success_message {
            self.ui.success_message = Some(msg);
        }
        if let Some(err) = sched_result.error_message {
            self.ui.error_message = Some(err);
        }

        // -- Buttons row -----------------------------------------------------
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let btn = egui::Button::new("Respaldar ahora")
                .fill(theme.btn_bg)
                .stroke(egui::Stroke::new(1.0, theme.border_color))
                .rounding(4.0);
            if ui
                .add(btn)
                .on_hover_text("Iniciar copia de seguridad ahora (Ctrl+B)")
                .clicked()
                && self.save_config()
            {
                self.start_backup();
            }

            if self.ui.backup_active {
                let cancel_btn = egui::Button::new("Cancelar")
                    .fill(theme.danger_bg)
                    .stroke(egui::Stroke::new(1.0, theme.danger_border))
                    .rounding(4.0);
                if ui
                    .add(cancel_btn)
                    .on_hover_text("Detener la copia en curso")
                    .clicked()
                {
                    self.ui.show_cancel_dialog = true;
                }
            }
        });

        // -- Progress --------------------------------------------------------
        panels::progress_panel::render(ui, &self.runner, &theme);

        // -- Save configuration button ---------------------------------------
        ui.add_space(12.0);
        ui.add(egui::Separator::default());
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            let save_btn = egui::Button::new("Guardar configuración")
                .fill(theme.btn_bg)
                .stroke(egui::Stroke::new(1.0, theme.border_color))
                .rounding(4.0);
            if ui
                .add(save_btn)
                .on_hover_text("Guardar rutas y preferencias actuales (Ctrl+S)")
                .clicked()
            {
                if self.save_config() {
                    self.ui.success_message = Some("Configuración guardada".to_string());
                } else {
                    self.ui.error_message = Some("Error al guardar configuración".to_string());
                }
            }

            // Inline feedback
            if let Some(saved) = self.ui.config_saved_at {
                if (Local::now() - saved).num_milliseconds() < 1500 {
                    ui.colored_label(theme.success_color, "✓ Guardado");
                }
            }
        });

        // -- Versions --------------------------------------------------------
        panels::versions_panel::render(
            ui,
            &mut self.version_mgr,
            &mut self.ui.pending_delete,
            &theme,
        );

        // -- Error / Success messages ----------------------------------------
        panels::messages::render(
            ui,
            &mut self.ui.error_message,
            &mut self.ui.success_message,
            &theme,
        );

        // -- Footer / Status bar ---------------------------------------------
        panels::status_bar::render(
            ui,
            self.ui.backup_active,
            self.schedule_enabled,
            &self.ui.last_backup_time,
            &self.ui.next_backup_display,
            &theme,
        );
    }
}
