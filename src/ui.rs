use chrono::Timelike;
use eframe::egui::{self, Widget};
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
                        self.next_backup_display = Some(format!("hoy {:02}:{:02}", h, m));
                    } else {
                        self.next_backup_display = Some(format!("mañana {:02}:{:02}", h, m));
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
}

impl eframe::App for AutoCopyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_next_backup_display();

        if self.schedule_enabled && !self.scheduling_active {
            let schedule_time = self.schedule_time.clone();
            let max_versions = self.max_versions;

            self.scheduling_active = true;

            thread::spawn(move || {
                let mut last_executed_day = String::new();

                loop {
                    thread::sleep(Duration::from_secs(30));

                    let now = chrono::Local::now();
                    let current_time = format!("{:02}:{:02}", now.hour(), now.minute());
                    let current_day = now.format("%Y-%m-%d").to_string();

                    if current_time == schedule_time && current_day != last_executed_day {
                        last_executed_day = current_day.clone();

                        let config = crate::config::AppConfig::load();
                        let source = config.last_source.clone();
                        let dest = config.last_dest.clone();

                        if source.is_none() || dest.is_none() {
                            continue;
                        }

                        let (tx, _rx) = mpsc::channel();
                        let cancel_flag = Arc::new(AtomicBool::new(false));

                        let source = source.unwrap();
                        let dest = dest.unwrap();

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
        // Colors from design system
        let brand_color = egui::Color32::from_rgb(0, 102, 255);
        let surface_dim = egui::Color32::from_rgb(210, 217, 244);
        let text_primary = egui::Color32::from_rgb(51, 51, 51);
        let text_secondary = egui::Color32::from_rgb(100, 100, 100);
        let border_color = egui::Color32::from_rgb(193, 213, 225);
        let success_color = egui::Color32::from_rgb(44, 203, 91);
        let btn_bg = egui::Color32::from_rgb(229, 231, 235);

        ui.add_space(12.0);

        // Title
        ui.label(
            egui::RichText::new("AutoCopy - Respaldo con Versionado")
                .heading()
                .color(text_primary),
        );
        ui.add_space(16.0);

        // SECTION: Path Configuration
        ui.label(egui::RichText::new("Origen:").color(text_secondary));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let source_text = self
                .source_path
                .clone()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut text = source_text;
            ui.text_edit_singleline(&mut text);
            if !text.is_empty() {
                self.source_path = Some(PathBuf::from(text));
            }
            if ui
                .add(
                    egui::Button::new("Browse...")
                        .fill(btn_bg)
                        .stroke(egui::Stroke::new(1.0, border_color))
                        .rounding(4.0),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.source_path = Some(path);
                }
            }
        });

        ui.add_space(12.0);

        ui.label(egui::RichText::new("Destino:").color(text_secondary));
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let dest_text = self
                .dest_path
                .clone()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut text = dest_text;
            ui.text_edit_singleline(&mut text);
            if !text.is_empty() {
                self.dest_path = Some(PathBuf::from(text));
            }
            if ui
                .add(
                    egui::Button::new("Browse...")
                        .fill(btn_bg)
                        .stroke(egui::Stroke::new(1.0, border_color))
                        .rounding(4.0),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.dest_path = Some(path);
                    self.refresh_versions();
                }
            }
        });

        // Divider
        ui.add_space(16.0);
        ui.add(egui::Separator::default());
        ui.add_space(16.0);

        // SECTION: Version Settings
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Máx versiones:").color(text_secondary));
            ui.add_space(8.0);
            egui::DragValue::new(&mut self.max_versions)
                .range(3..=10)
                .ui(ui);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("(3-10)").small().color(text_secondary));
        });

        // Divider
        ui.add_space(16.0);
        ui.add(egui::Separator::default());
        ui.add_space(16.0);

        // SECTION: Schedule Settings
        ui.label(
            egui::RichText::new("Programar respaldo automático")
                .heading()
                .color(text_primary),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.schedule_enabled, "");
            ui.label(egui::RichText::new("Habilitado").color(text_primary));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Hora:").color(text_secondary));
            ui.add_space(8.0);

            let old_time = self.schedule_time.clone();
            ui.text_edit_singleline(&mut self.schedule_time);

            if self.schedule_time != old_time {
                self.update_next_backup_display();
                self.save_config();
                self.scheduling_active = false;
            }

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("(formato 24h, ej: 14:30)")
                    .small()
                    .color(text_secondary),
            );
        });

        // Schedule info
        ui.add_space(8.0);

        if let Some(next) = &self.next_backup_display {
            ui.label(
                egui::RichText::new(format!("Próximo respaldo: {}", next)).color(text_secondary),
            );
        }

        if self.scheduling_active && !self.is_backup_running {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Scheduling activo - esperando hora programada")
                    .color(text_secondary),
            );
        }

        ui.add_space(12.0);

        // Buttons row
        ui.horizontal(|ui| {
            let btn = egui::Button::new("Respaldar ahora")
                .fill(btn_bg)
                .stroke(egui::Stroke::new(1.0, border_color))
                .rounding(4.0);
            if ui.add(btn).clicked() {
                if self.save_config() {
                    self.start_backup();
                }
            }

            if self.is_backup_running {
                let cancel_btn = egui::Button::new("Cancelar")
                    .fill(egui::Color32::from_rgb(255, 200, 200))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(200, 100, 100),
                    ))
                    .rounding(4.0);
                if ui.add(cancel_btn).clicked() {
                    self.cancel_backup();
                }
            }
        });

        // Progress section
        if self.progress.started && !self.progress.finished {
            ui.add_space(16.0);
            ui.add(egui::Separator::default());
            ui.add_space(8.0);

            ui.label(egui::RichText::new("Progreso:").color(text_primary));
            ui.add_space(8.0);

            let fraction = if self.progress.total_files > 0 {
                self.progress.current_index as f32 / self.progress.total_files as f32
            } else {
                0.0
            };

            ui.add(
                egui::ProgressBar::new(fraction)
                    .fill(brand_color)
                    .rounding(12.0)
                    .text(format!(
                        "{}% ({}/{})",
                        (fraction * 100.0) as usize,
                        self.progress.current_index,
                        self.progress.total_files
                    )),
            );

            if !self.progress.current_file.is_empty() {
                ui.label(
                    egui::RichText::new(format!("Copiando: {}", self.progress.current_file))
                        .color(text_secondary),
                );
            }
        }

        // Divider
        ui.add_space(16.0);
        ui.add(egui::Separator::default());
        ui.add_space(16.0);

        // Save configuration button
        let save_btn = egui::Button::new("Guardar configuración")
            .fill(btn_bg)
            .stroke(egui::Stroke::new(1.0, border_color))
            .rounding(4.0);
        if ui.add(save_btn).clicked() {
            self.save_config();
            if self.config_save_success.unwrap_or(false) {
                self.success_message = Some("Configuración guardada".to_string());
            } else {
                self.error_message = Some("Error al guardar configuración".to_string());
            }
            self.config_save_success = None;
        }

        // Divider
        ui.add_space(16.0);
        ui.add(egui::Separator::default());
        ui.add_space(16.0);

        // SECTION: Versions
        ui.label(
            egui::RichText::new("Versiones guardadas:")
                .heading()
                .color(text_primary),
        );
        ui.add_space(8.0);

        if self.versions.is_empty() {
            ui.label(
                egui::RichText::new("No hay versiones guardadas.")
                    .italics()
                    .color(text_secondary),
            );
        } else {
            for version in &self.versions {
                ui.horizontal(|ui| {
                    if let Some(name) = version.file_name() {
                        ui.label(name.to_string_lossy());
                    }
                    let open_btn = egui::Button::new("Abrir")
                        .fill(btn_bg)
                        .stroke(egui::Stroke::new(1.0, border_color))
                        .rounding(4.0);
                    if ui.add(open_btn).clicked() {
                        self.open_in_explorer(version);
                    }
                });
            }
        }

        if let Some(last) = &self.last_backup_time {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("Último respaldo: {}", last)).color(text_secondary),
            );
        }

        // Error/Success messages
        if let Some(err) = self.error_message.clone() {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, err);
            if ui.button("Aceptar").clicked() {
                self.error_message = None;
            }
        }

        if let Some(msg) = self.success_message.clone() {
            ui.add_space(8.0);
            ui.colored_label(success_color, msg);
            if ui.button("Aceptar").clicked() {
                self.success_message = None;
            }
        }

        // Footer
        ui.add_space(24.0);
        ui.add(egui::Separator::default());
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Desarrollado por JhosenthG")
                .small()
                .color(egui::Color32::from_rgb(180, 180, 180)),
        );
    }
}
