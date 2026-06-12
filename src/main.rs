use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

mod backup_runner;
mod config;
mod copy;
mod error;
mod scheduler;
mod theme;
mod ui;
mod version_manager;

use config::AppConfig;
use copy::{BackupOptions, perform_backup, validate_paths};

fn get_log_dir() -> PathBuf {
    let app_data = env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(app_data).join("autocopy").join("logs")
}

fn log_message(message: &str) {
    if let Some(log_dir) = get_log_dir().to_str() {
        let log_path = PathBuf::from(log_dir).join("autocopy.log");
        fs::create_dir_all(log_path.parent().unwrap_or(&log_path)).ok();
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            writeln!(file, "[{}] {}", timestamp, message).ok();
        }
    }
}

fn run_cli_backup() {
    log_message("CLI backup mode started");

    let config = AppConfig::load();

    let source = match &config.last_source {
        Some(p) => p.clone(),
        None => {
            log_message("Error: No source path configured");
            eprintln!(
                "Error: No has configurado las rutas de respaldo.\n\
                 Para configurar, ejecuta la aplicación sin argumentos:\n\
                 \n  autocopy.exe\n\
                 \ny selecciona las carpetas Origen y Destino en la interfaz gráfica.\n\
                 La configuración se guarda en: {}\\autocopy\\config.json",
                std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())
            );
            std::process::exit(1);
        }
    };

    let dest = match &config.last_dest {
        Some(p) => p.clone(),
        None => {
            log_message("Error: No destination path configured");
            eprintln!(
                "Error: No has configurado las rutas de respaldo.\n\
                 Para configurar, ejecuta la aplicación sin argumentos:\n\
                 \n  autocopy.exe\n\
                 \ny selecciona las carpetas Origen y Destino en la interfaz gráfica.\n\
                 La configuración se guarda en: {}\\autocopy\\config.json",
                std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())
            );
            std::process::exit(1);
        }
    };

    log_message(&format!("Source: {}", source.display()));
    log_message(&format!("Dest: {}", dest.display()));

    if let Err(e) = validate_paths(&source, &dest) {
        log_message(&format!("Validation error: {}", e));
        eprintln!("Validation error: {}", e);
        std::process::exit(1);
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, _rx) = mpsc::channel();

    let opts = BackupOptions {
        cancel_flag,
        progress_tx: tx,
    };

    let max_versions = config.max_versions;

    match perform_backup(&source, &dest, opts) {
        Ok(backup_path) => {
            log_message(&format!("Backup created at: {}", backup_path.display()));
            if let Err(e) = copy::cleanup_old_versions(&dest, max_versions) {
                log_message(&format!("Cleanup warning: {}", e));
            }
            println!("Backup completed successfully: {}", backup_path.display());
        }
        Err(e) => {
            log_message(&format!("Backup error: {}", e));
            eprintln!("Backup error: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_gui() {
    log_message("GUI mode started");

    // Load the favicon as the application icon
    let icon = {
        let png_bytes = include_bytes!("../icons/icon_autocopy_favicon.PNG");
        image::load_from_memory(png_bytes).ok().map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            eframe::egui::IconData {
                rgba: rgba.into_raw(),
                width: w,
                height: h,
            }
        })
    };

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([700.0, 600.0])
        .with_min_inner_size([640.0, 480.0])
        .with_title("AutoCopy - Respaldo con Versionado");
    if let Some(ic) = icon {
        viewport = viewport.with_icon(ic);
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "AutoCopy",
        native_options,
        Box::new(|_cc| Ok(Box::new(ui::AutoCopyApp::new()))),
    )
    .expect("Failed to start eframe");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--backup".to_string()) || args.contains(&"-b".to_string()) {
        run_cli_backup();
    } else {
        run_gui();
    }
}
