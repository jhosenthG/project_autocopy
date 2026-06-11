use chrono::{DateTime, Local, Timelike};
use eframe::egui::{self, Widget};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use crate::backup_runner::BackupRunner;
use crate::config::AppConfig;
use crate::copy;
use crate::scheduler;
use crate::theme::AppTheme;
use crate::version_manager::{self, SortOrder, VersionManager};

use super::components::{format_size, open_in_explorer, path_row};

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

    // -- UI state ------------------------------------------------------------
    backup_active: bool,
    error_message: Option<String>,
    success_message: Option<String>,
    last_backup_time: Option<String>,
    scheduling_active: bool,
    scheduler_cancel: Arc<AtomicBool>,
    next_backup_display: Option<String>,
    source_valid: Option<bool>,
    dest_valid: Option<bool>,
    show_cancel_dialog: bool,
    success_timer: Option<DateTime<Local>>,
    pending_delete: Option<PathBuf>,
    config_saved_at: Option<DateTime<Local>>,
    winsched_active: bool,
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
            backup_active: false,
            error_message: None,
            success_message: None,
            last_backup_time: None,
            scheduling_active: false,
            scheduler_cancel: Arc::new(AtomicBool::new(false)),
            next_backup_display: None,
            source_valid: None,
            dest_valid: None,
            show_cancel_dialog: false,
            success_timer: None,
            pending_delete: None,
            config_saved_at: None,
            winsched_active: winsched,
        }
    }

    // -- Path validation -----------------------------------------------------

    fn validate_path_fields(&mut self) {
        self.source_valid = match &self.source_path {
            Some(p) if !p.as_os_str().is_empty() => Some(p.exists()),
            _ => None,
        };
        self.dest_valid = match &self.dest_path {
            Some(p) if !p.as_os_str().is_empty() => Some(
                p.exists() && {
                    self.source_path
                        .as_ref()
                        .map(|s| s.as_os_str() != p.as_os_str())
                        .unwrap_or(true)
                },
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
                self.config_saved_at = Some(Local::now());
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
                self.error_message = Some("Por favor selecciona una carpeta origen.".to_string());
                return;
            }
        };
        let dest = match &self.dest_path {
            Some(p) => p.clone(),
            None => {
                self.error_message = Some("Por favor selecciona una carpeta destino.".to_string());
                return;
            }
        };

        // Validate paths and available space before starting
        if let Err(e) = copy::validate_paths(&source, &dest) {
            self.error_message = Some(format!("Error de validación: {}", e));
            return;
        }

        self.runner.start(source, dest, self.max_versions);
        self.backup_active = true;
        self.scheduling_active = true;
    }

    fn cancel_backup(&mut self) {
        self.runner.cancel();
        self.backup_active = false;
        self.scheduling_active = false;
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

                    self.next_backup_display = Some(if schedule_minutes > current_minutes {
                        format!("hoy {:02}:{:02}", h, m)
                    } else {
                        format!("mañana {:02}:{:02}", h, m)
                    });
                    return;
                }
            }
        }
        self.next_backup_display = None;
    }
}

// ===========================================================================
//  eframe::App trait implementation (main update loop)
// ===========================================================================
impl eframe::App for AutoCopyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_next_backup_display();
        self.validate_path_fields();

        // Auto-dismiss success messages after 3 seconds
        if let Some(timer) = self.success_timer {
            if (Local::now() - timer).num_seconds() >= 3 {
                self.success_message = None;
                self.success_timer = None;
            }
        }

        // Auto-clear config_saved_at after 1.5 seconds
        if let Some(saved) = self.config_saved_at {
            if (Local::now() - saved).num_milliseconds() >= 1500 {
                self.config_saved_at = None;
            }
        }

        // Keyboard shortcuts
        {
            let do_backup = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::B));
            if do_backup && self.save_config() {
                self.start_backup();
            }

            let do_save = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S));
            if do_save {
                self.save_config();
            }

            let do_escape =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if do_escape {
                self.error_message = None;
                self.success_message = None;
                self.show_cancel_dialog = false;
                self.pending_delete = None;
            }
        }

        // Drag & drop folders
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            for file in &dropped {
                if let Some(path) = &file.path {
                    if path.is_dir() {
                        if self.source_path.is_none() {
                            self.source_path = Some(path.clone());
                        } else if self.dest_path.is_none() {
                            self.dest_path = Some(path.clone());
                            self.version_mgr.set_dest(Some(path.clone()));
                            self.version_mgr.refresh();
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

        // In-app scheduler thread
        if self.schedule_enabled && !self.scheduling_active {
            let schedule_time = self.schedule_time.clone();
            let max_versions = self.max_versions;

            self.scheduling_active = true;
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
            self.scheduling_active = false;
        }

        // Poll backup progress
        if self.runner.poll() {
            self.backup_active = false;
            self.scheduling_active = false;
            self.success_message = Some("Respaldo completado exitosamente.".to_string());
            self.last_backup_time = Some(Local::now().format("%Y-%m-%d %H:%M").to_string());
            if let Some(dest) = &self.dest_path {
                self.version_mgr.set_dest(Some(dest.clone()));
                self.version_mgr.refresh();
            }
        }

        // -- Dialogs ---------------------------------------------------------

        // Cancel confirmation dialog
        if self.show_cancel_dialog {
            egui::Window::new("Cancelar respaldo")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("¿Estás seguro de que deseas cancelar el respaldo?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Sí, cancelar").clicked() {
                            self.cancel_backup();
                            self.show_cancel_dialog = false;
                        }
                        if ui.button("Continuar").clicked() {
                            self.show_cancel_dialog = false;
                        }
                    });
                });
        }

        // Delete version confirmation dialog
        if let Some(path) = &self.pending_delete.clone() {
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
                            match self.version_mgr.delete_version(path) {
                                Ok(()) => {
                                    self.success_message =
                                        Some("Versión eliminada correctamente.".to_string());
                                }
                                Err(_) => {
                                    self.error_message =
                                        Some("Error al eliminar la versión.".to_string());
                                }
                            }
                            self.pending_delete = None;
                        }
                        if ui.button("Cancelar").clicked() {
                            self.pending_delete = None;
                        }
                    });
                });
        }

        // -- Main UI ---------------------------------------------------------
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_ui(ui);
        });
    }
}

// ===========================================================================
//  UI Rendering
// ===========================================================================
impl AutoCopyApp {
    #[allow(clippy::too_many_lines)]
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        let theme = AppTheme::from_visuals(&ui.style().visuals);

        ui.add_space(12.0);

        // -- Title -----------------------------------------------------------
        ui.label(
            egui::RichText::new("AutoCopy - Respaldo con Versionado")
                .heading()
                .color(theme.text_primary),
        );
        ui.add_space(16.0);

        // --------------------------------------------------------------------
        // SECTION: Path Configuration
        // --------------------------------------------------------------------
        let _picked_source = path_row(
            ui,
            "Origen:",
            &mut self.source_path,
            self.source_valid,
            &theme,
        );

        ui.add_space(12.0);

        let picked_dest = path_row(ui, "Destino:", &mut self.dest_path, self.dest_valid, &theme);

        if picked_dest {
            if let Some(ref dest) = self.dest_path {
                self.version_mgr.set_dest(Some(dest.clone()));
                self.version_mgr.refresh();
            }
        }

        // Available space on destination
        if let Some(ref dest) = self.dest_path {
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

        // --------------------------------------------------------------------
        // SECTION: Version Settings
        // --------------------------------------------------------------------
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

        // --------------------------------------------------------------------
        // SECTION: Schedule Settings
        // --------------------------------------------------------------------
        ui.label(
            egui::RichText::new("Programar respaldo automático")
                .heading()
                .color(theme.text_primary),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.schedule_enabled, "")
                .on_hover_text("Activar respaldo automático diario")
                .clicked()
            {
                self.save_config();
                if self.schedule_enabled {
                    self.scheduling_active = false;
                }
            }
            ui.label(egui::RichText::new("Activar respaldo automático").color(theme.text_primary));
        });

        ui.add_space(8.0);

        // Time picker
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Hora:").color(theme.text_secondary));
            ui.add_space(8.0);

            let old_h = self.schedule_hour;
            let old_m = self.schedule_minute;

            ui.add(
                egui::Slider::new(&mut self.schedule_hour, 0..=23)
                    .text("h")
                    .clamping(egui::SliderClamping::Always),
            )
            .on_hover_text("Hora del respaldo automático (formato 24h)");

            ui.label(":");

            ui.add(
                egui::Slider::new(&mut self.schedule_minute, 0..=59)
                    .text("m")
                    .clamping(egui::SliderClamping::Always),
            )
            .on_hover_text("Minuto del respaldo automático");

            if self.schedule_hour != old_h || self.schedule_minute != old_m {
                self.schedule_time =
                    format!("{:02}:{:02}", self.schedule_hour, self.schedule_minute);
                self.save_config();
                self.scheduling_active = false;
            }
        });

        ui.add_space(8.0);

        if let Some(next) = &self.next_backup_display {
            ui.label(
                egui::RichText::new(format!("Próximo respaldo: {}", next))
                    .color(theme.text_secondary),
            );
        }

        if self.scheduling_active && !self.backup_active {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Monitor de horario activo — esperando hora programada")
                    .color(theme.text_secondary),
            );
        }

        // Windows Task Scheduler integration
        if self.schedule_enabled {
            ui.add_space(4.0);
            if self.winsched_active {
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
                        if scheduler::unschedule_backup_task().is_ok() {
                            self.winsched_active = false;
                            self.success_message = Some("Tarea programada eliminada.".to_string());
                        } else {
                            self.error_message =
                                Some("Error al eliminar tarea de Windows.".to_string());
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
                        match scheduler::schedule_backup_task(&exe, &self.schedule_time) {
                            Ok(()) => {
                                self.winsched_active = true;
                                self.success_message =
                                    Some("Tarea programada creada en Windows.".to_string());
                            }
                            Err(e) => {
                                self.error_message = Some(format!("Error al crear tarea: {}", e));
                            }
                        }
                    }
                });
            }
        }

        ui.add_space(12.0);

        // --------------------------------------------------------------------
        // Buttons row
        // --------------------------------------------------------------------
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

            if self.backup_active {
                let cancel_btn = egui::Button::new("Cancelar")
                    .fill(theme.danger_bg)
                    .stroke(egui::Stroke::new(1.0, theme.danger_border))
                    .rounding(4.0);
                if ui
                    .add(cancel_btn)
                    .on_hover_text("Detener la copia en curso")
                    .clicked()
                {
                    self.show_cancel_dialog = true;
                }
            }
        });

        // --------------------------------------------------------------------
        // Progress section
        // --------------------------------------------------------------------
        if self.runner.progress.started && !self.runner.progress.finished {
            ui.add_space(12.0);
            ui.add(egui::Separator::default());
            ui.add_space(8.0);

            ui.label(egui::RichText::new("Progreso:").color(theme.text_primary));
            ui.add_space(8.0);

            let fraction = if self.runner.progress.total_files > 0 {
                (self.runner.progress.current_index as f32)
                    .min(self.runner.progress.total_files as f32)
                    / self.runner.progress.total_files as f32
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
                        self.runner.progress.current_index,
                        self.runner.progress.total_files
                    )),
            );

            if !self.runner.progress.current_file.is_empty() {
                ui.label(
                    egui::RichText::new(format!("Copiando: {}", self.runner.progress.current_file))
                        .color(theme.text_secondary),
                );
            }

            if let Some((speed_bytes, eta_secs)) = self.runner.compute_eta() {
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

        ui.add_space(12.0);
        ui.add(egui::Separator::default());
        ui.add_space(12.0);

        // --------------------------------------------------------------------
        // Save configuration button
        // --------------------------------------------------------------------
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
                    self.success_message = Some("Configuración guardada".to_string());
                } else {
                    self.error_message = Some("Error al guardar configuración".to_string());
                }
            }

            // Inline feedback
            if let Some(saved) = self.config_saved_at {
                if (Local::now() - saved).num_milliseconds() < 1500 {
                    ui.colored_label(theme.success_color, "✓ Guardado");
                }
            }
        });

        ui.add_space(12.0);
        ui.add(egui::Separator::default());
        ui.add_space(12.0);

        // --------------------------------------------------------------------
        // SECTION: Versions
        // --------------------------------------------------------------------
        ui.label(
            egui::RichText::new("Versiones guardadas:")
                .heading()
                .color(theme.text_primary),
        );
        ui.add_space(8.0);

        // Sort & filter controls
        if !self.version_mgr.versions.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Orden:")
                        .small()
                        .color(theme.text_secondary),
                );
                egui::ComboBox::from_id_salt("sort_order")
                    .selected_text(match self.version_mgr.sort_order {
                        SortOrder::Newest => "Más recientes",
                        SortOrder::Oldest => "Más antiguas",
                        SortOrder::Largest => "Más grandes",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.version_mgr.sort_order,
                            SortOrder::Newest,
                            "Más recientes",
                        );
                        ui.selectable_value(
                            &mut self.version_mgr.sort_order,
                            SortOrder::Oldest,
                            "Más antiguas",
                        );
                        ui.selectable_value(
                            &mut self.version_mgr.sort_order,
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
                let mut filter = self.version_mgr.filter.clone();
                let filter_resp = ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .desired_width(120.0)
                        .hint_text("buscar..."),
                );
                if filter_resp.changed() {
                    self.version_mgr.filter = filter;
                    self.version_mgr.refresh();
                } else if filter != self.version_mgr.filter {
                    self.version_mgr.refresh();
                }
            });
            ui.add_space(4.0);
        }

        if self.version_mgr.versions.is_empty() {
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
                    for version in &self.version_mgr.versions.clone() {
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
                                self.pending_delete = Some(version.clone());
                            }
                        });
                    }
                });
        }

        // --------------------------------------------------------------------
        // Error / Success messages
        // --------------------------------------------------------------------
        if let Some(err) = self.error_message.clone() {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, err);
            if ui.button("Aceptar").clicked() {
                self.error_message = None;
            }
        }

        if let Some(msg) = self.success_message.clone() {
            ui.add_space(8.0);
            ui.colored_label(theme.success_color, msg);
            if ui.button("Aceptar").clicked() {
                self.success_message = None;
            }
        }

        // --------------------------------------------------------------------
        // Footer / Status bar
        // --------------------------------------------------------------------
        ui.add_space(16.0);
        ui.add(egui::Separator::default());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Estado:")
                    .small()
                    .color(theme.text_secondary),
            );

            let status = if self.backup_active {
                "Respaldando...".to_string()
            } else if self.schedule_enabled {
                if let Some(next) = &self.next_backup_display {
                    format!("Programado: {}", next)
                } else {
                    "Programado: —".to_string()
                }
            } else {
                "Inactivo".to_string()
            };
            ui.colored_label(theme.text_secondary, status);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(last) = &self.last_backup_time {
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
}
