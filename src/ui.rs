use chrono::Timelike;
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::AppConfig;
use crate::copy::{list_versions, ProgressEvent};

pub struct AutoCopyApp {
    pub config: AppConfig,
    pub source_path: Option<PathBuf>,
    pub dest_path: Option<PathBuf>,
    pub max_versions: usize,
    pub schedule_enabled: bool,
    pub schedule_time: String,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub versions: Vec<PathBuf>,
    pub is_backup_running: bool,
    pub progress: BackupProgress,
    pub last_backup_time: Option<String>,
    pub progress_receiver: Option<mpsc::Receiver<ProgressEvent>>,
    pub cancel_flag: std::sync::Arc<AtomicBool>,
    scheduling_active: bool,
    next_backup_display: Option<String>,
    config_save_success: Option<bool>,
}

#[derive(Default, Clone)]
pub struct BackupProgress {
    pub total_files: usize,
    pub total_bytes: u64,
    pub current_file: String,
    pub current_index: usize,
    pub bytes_copied: u64,
    pub started: bool,
    pub finished: bool,
}

impl AutoCopyApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        let mut app = Self {
            config: config.clone(),
            source_path: config.last_source,
            dest_path: config.last_dest,
            max_versions: config.max_versions,
            schedule_enabled: config.schedule_enabled,
            schedule_time: config
                .schedule_time
                .clone()
                .unwrap_or_else(|| "14:30".to_string()),
            error_message: None,
            success_message: None,
            versions: Vec::new(),
            is_backup_running: false,
            progress: BackupProgress::default(),
            last_backup_time: None,
            progress_receiver: None,
            cancel_flag: std::sync::Arc::new(AtomicBool::new(false)),
            scheduling_active: false,
            next_backup_display: None,
            config_save_success: None,
        };
        app.refresh_versions();
        app.update_next_backup_display();
        app
    }

    fn refresh_versions(&mut self) {
        if let Some(dest) = &self.dest_path {
            self.versions = list_versions(dest).unwrap_or_default();
        }
    }

    fn save_config(&mut self) -> bool {
        let mut config = self.config.clone();
        config.last_source = self.source_path.clone();
        config.last_dest = self.dest_path.clone();
        config.max_versions = self.max_versions;
        config.schedule_enabled = self.schedule_enabled;
        config.schedule_time = Some(self.schedule_time.clone());

        match config.save() {
            Ok(()) => {
                self.config_save_success = Some(true);
                true
            }
            Err(e) => {
                eprintln!("Failed to save config: {}", e);
                self.config_save_success = Some(false);
                false
            }
        }
    }

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

        self.cancel_flag = std::sync::Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        self.progress_receiver = Some(rx);

        self.is_backup_running = true;
        self.progress = BackupProgress::default();
        self.scheduling_active = true;

        let cancel_flag = self.cancel_flag.clone();
        let max_versions = self.max_versions;

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

    fn cancel_backup(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        self.is_backup_running = false;
        self.scheduling_active = false;
    }

    fn open_in_explorer(&self, path: &PathBuf) {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .ok();
        }
    }

    fn update_next_backup_display(&mut self) {
        if self.schedule_enabled {
            let now = chrono::Local::now();
            let current_minutes = now.hour() * 60 + now.minute();

            let schedule_parts: Vec<&str> = self.schedule_time.split(':').collect();
            if schedule_parts.len() == 2 {
                if let (Ok(h), Ok(m)) = (
                    schedule_parts[0].parse::<u32>(),
                    schedule_parts[1].parse::<u32>(),
                ) {
                    let schedule_minutes = h * 60 + m;

                    if schedule_minutes > current_minutes {
                        self.next_backup_display = Some(format!("hoy {}:{:02}", h, m));
                    } else {
                        self.next_backup_display = Some(format!("mañana {}:{:02}", h, m));
                    }
                    return;
                }
            }
        }
        self.next_backup_display = None;
    }

    fn poll_progress(&mut self) {
        let rx = match &self.progress_receiver {
            Some(rx) => Some(rx.try_iter().collect::<Vec<_>>()),
            None => None,
        };

        if let Some(events) = rx {
            for event in events {
                match event {
                    ProgressEvent::Started {
                        total_files,
                        total_bytes,
                    } => {
                        self.progress.started = true;
                        self.progress.total_files = total_files;
                        self.progress.total_bytes = total_bytes;
                    }
                    ProgressEvent::FileStarted { path, index } => {
                        self.progress.current_file = path.to_string_lossy().to_string();
                        self.progress.current_index = index;
                    }
                    ProgressEvent::FileCompleted { bytes_copied } => {
                        self.progress.bytes_copied += bytes_copied;
                    }
                    ProgressEvent::Finished => {
                        self.progress.finished = true;
                        self.is_backup_running = false;
                        self.scheduling_active = false;
                        self.success_message =
                            Some("Respaldo completado exitosamente.".to_string());
                        self.last_backup_time =
                            Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
                        let _ = self.dest_path.as_ref().map(|dest| {
                            if let Ok(versions) = crate::copy::list_versions(dest) {
                                self.versions = versions;
                            }
                        });
                    }
                }
            }
        }
    }

    fn check_and_execute_scheduled_backup(&mut self) {
        if !self.schedule_enabled {
            return;
        }
        if self.is_backup_running {
            return;
        }

        let now = chrono::Local::now();
        let current_time = format!("{:02}:{:02}", now.hour(), now.minute());

        if current_time == self.schedule_time {
            self.start_backup();
        }
    }
}

impl eframe::App for AutoCopyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_next_backup_display();

        // Spawnar thread de scheduling si está habilitado y no existe ya
        if self.schedule_enabled && !self.scheduling_active {
            let schedule_time = self.schedule_time.clone();
            let max_versions = self.max_versions;
            let ctx = ctx.clone();

            self.scheduling_active = true;

            thread::spawn(move || {
                let mut last_executed_day = String::new();

                loop {
                    thread::sleep(Duration::from_secs(30));

                    let now = chrono::Local::now();
                    let current_time = format!("{:02}:{:02}", now.hour(), now.minute());
                    let current_day = now.format("%Y-%m-%d").to_string();

                    // Solo ejecutar una vez por día a la hora programada
                    if current_time == schedule_time && current_day != last_executed_day {
                        last_executed_day = current_day.clone();

                        // Obtener config actual para usar valores vigentes
                        let config = crate::config::AppConfig::load();

                        let source = config.last_source.clone();
                        let dest = config.last_dest.clone();

                        if source.is_none() || dest.is_none() {
                            eprintln!("[Scheduler] No hay source/dest configurados, saltando...");
                            continue;
                        }

                        eprintln!(
                            "[Scheduler] Ejecutando backup programado para {} a {}",
                            current_day, current_time
                        );

                        let (tx, _rx) = mpsc::channel();
                        let cancel_flag = Arc::new(AtomicBool::new(false));

                        let source = source.unwrap();
                        let dest = dest.unwrap();

                        thread::spawn(move || {
                            let opts = crate::copy::BackupOptions {
                                cancel_flag,
                                progress_tx: tx,
                            };

                            eprintln!(
                                "[Scheduler] Backup iniciado: {} -> {}",
                                source.display(),
                                dest.display()
                            );

                            let result = crate::copy::perform_backup(&source, &dest, opts);

                            match result {
                                Ok(backup_path) => {
                                    eprintln!(
                                        "[Scheduler] Backup completado: {}",
                                        backup_path.display()
                                    );
                                    let _ = crate::copy::cleanup_old_versions(&dest, max_versions);
                                }
                                Err(e) => {
                                    eprintln!("[Scheduler] Backup falló: {}", e);
                                }
                            }
                        });
                    }
                }
            });
        }

        // Verificar si scheduling está habilitado pero necesitamos reiniciar el thread
        // (ej: cambió la hora o se habilitó después de iniciar)
        if !self.schedule_enabled {
            self.scheduling_active = false;
        }

        self.poll_progress();

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_ui(ui);
        });
    }
}

impl AutoCopyApp {
    pub fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("AutoCopy - Respaldo con Versionado");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Origen:");
            let source_text = self
                .source_path
                .clone()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut text = source_text;
            if ui.text_edit_singleline(&mut text).changed() {
                if !text.is_empty() {
                    self.source_path = Some(PathBuf::from(text));
                }
            }
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.source_path = Some(path);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Destino:");
            let dest_text = self
                .dest_path
                .clone()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut text = dest_text;
            if ui.text_edit_singleline(&mut text).changed() {
                if !text.is_empty() {
                    self.dest_path = Some(PathBuf::from(text));
                }
            }
            if ui.button("Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.dest_path = Some(path);
                    self.refresh_versions();
                }
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Máx versiones:");
            ui.add(egui::DragValue::new(&mut self.max_versions).range(3..=10));
            ui.label("(3-10)");
        });

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Programar respaldo automático");

        let schedule_changed = ui
            .checkbox(&mut self.schedule_enabled, "Habilitado")
            .changed();

        let old_time = self.schedule_time.clone();
        ui.horizontal(|ui| {
            ui.label("Hora:");
            let mut text_edit = self.schedule_time.clone();
            ui.text_edit_singleline(&mut text_edit);
            self.schedule_time = text_edit;
            ui.label("(formato 24h, ej: 14:30)");
        });
        let time_changed = self.schedule_time != old_time;

        if schedule_changed && self.schedule_enabled {
            self.save_config();
            self.update_next_backup_display();
        }

        if time_changed {
            self.update_next_backup_display();
            self.save_config();
            self.scheduling_active = false;
        }

        if let Some(next) = &self.next_backup_display {
            ui.label(format!("Próximo respaldo: {}", next));
        }

        if self.scheduling_active && !self.is_backup_running {
            ui.label("✓ Scheduling activo - esperando hora programada");
        }

        ui.add_space(10.0);

        if ui
            .add_enabled(
                !self.is_backup_running,
                egui::Button::new("Respaldar ahora"),
            )
            .clicked()
        {
            if self.save_config() {
                self.start_backup();
            }
        }

        if self.is_backup_running {
            if ui.button("Cancelar").clicked() {
                self.cancel_backup();
            }
        }

        ui.add_space(10.0);
        ui.separator();

        // Botón guardar configuración
        ui.add_space(5.0);
        if ui.button("Guardar configuración").clicked() {
            self.save_config();
            // Mostrar feedback por 3 segundos y luego limpiar
            let success = self.config_save_success.unwrap_or(false);
            if success {
                self.success_message = Some("Configuración guardada".to_string());
            } else {
                self.error_message = Some("Error al guardar configuración".to_string());
            }
            self.config_save_success = None;
        }

        if self.progress.started && !self.progress.finished {
            ui.add_space(10.0);
            ui.separator();
            ui.heading("Progreso:");

            let fraction = if self.progress.total_files > 0 {
                self.progress.current_index as f32 / self.progress.total_files as f32
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(fraction).text(format!(
                "{}% ({}/{})",
                (fraction * 100.0) as usize,
                self.progress.current_index,
                self.progress.total_files
            )));

            if !self.progress.current_file.is_empty() {
                ui.label(format!("Copiando: {}", self.progress.current_file));
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Versiones guardadas:");

        for version in &self.versions {
            ui.horizontal(|ui| {
                if let Some(name) = version.file_name() {
                    ui.label(name.to_string_lossy());
                }
                if ui.button("Abrir").clicked() {
                    self.open_in_explorer(version);
                }
            });
        }

        if self.versions.is_empty() {
            ui.label("No hay versiones guardadas.");
        }

        if let Some(last) = &self.last_backup_time {
            ui.add_space(5.0);
            ui.label(format!("Último respaldo: {}", last));
        }

        ui.add_space(10.0);

        if let Some(err) = &self.error_message.clone() {
            ui.colored_label(egui::Color32::RED, err);
            if ui.button("Aceptar").clicked() {
                self.error_message = None;
            }
        }

        if let Some(msg) = &self.success_message.clone() {
            ui.colored_label(egui::Color32::GREEN, msg);
            if ui.button("Aceptar").clicked() {
                self.success_message = None;
            }
        }

        // Firma del desarrollador
        ui.add_space(20.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Desarrollado por JhosenthG")
                    .small()
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );
        });
    }
}
