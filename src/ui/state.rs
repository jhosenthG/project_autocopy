use chrono::{DateTime, Local};
use eframe::egui;
use std::path::PathBuf;

/// State that is exclusively used for UI presentation.
///
/// Separated from `AutoCopyApp` to keep domain state and presentation
/// concerns decoupled. This struct can be manipulated without touching
/// the application's core logic.
pub struct UiState {
    pub backup_active: bool,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub last_backup_time: Option<String>,
    /// True while the scheduler background thread is alive
    pub scheduler_running: bool,
    pub next_backup_display: Option<String>,
    pub source_valid: Option<bool>,
    pub dest_valid: Option<bool>,
    pub show_cancel_dialog: bool,
    pub success_timer: Option<DateTime<Local>>,
    pub pending_delete: Option<PathBuf>,
    pub config_saved_at: Option<DateTime<Local>>,
    pub winsched_active: bool,
    pub logo: Option<egui::TextureHandle>,

    // -- Dirty flags & caches for I/O debouncing --------------------------
    /// Set to `true` when source or dest path changes; reset after validation
    pub paths_dirty: bool,
    /// Cached available space + timestamp when it was fetched
    pub space_cache: Option<(u64, DateTime<Local>)>,
    /// Last parsed schedule_time value (avoids re-parsing every frame)
    pub last_schedule_time_parsed: Option<String>,
}

impl UiState {
    pub fn new(winsched_active: bool) -> Self {
        Self {
            backup_active: false,
            error_message: None,
            success_message: None,
            last_backup_time: None,
            scheduler_running: false,
            next_backup_display: None,
            source_valid: None,
            dest_valid: None,
            show_cancel_dialog: false,
            success_timer: None,
            pending_delete: None,
            config_saved_at: None,
            winsched_active,
            logo: None,
            paths_dirty: true,
            space_cache: None,
            last_schedule_time_parsed: None,
        }
    }
}
