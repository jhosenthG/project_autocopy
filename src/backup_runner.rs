use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use chrono::{DateTime, Local};

use crate::copy::{self, BackupOptions, ProgressEvent};

/// Tracks progress of an active or completed backup operation.
#[derive(Default, Clone)]
pub struct BackupProgress {
    pub total_files: usize,
    pub total_bytes: u64,
    pub current_file: String,
    pub current_index: usize,
    pub bytes_copied: u64,
    pub started: bool,
    pub finished: bool,
    pub start_time: Option<DateTime<Local>>,
}

/// Manages the lifecycle, cancellation, and progress reporting of a backup.
///
/// Encapsulates the background thread, progress channel, and cancellation flag
/// so the UI layer only sees a simple polling interface.
pub struct BackupRunner {
    pub progress: BackupProgress,
    cancel_flag: Arc<AtomicBool>,
    receiver: Option<mpsc::Receiver<ProgressEvent>>,
}

impl Default for BackupRunner {
    fn default() -> Self {
        Self {
            progress: BackupProgress::default(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            receiver: None,
        }
    }
}

impl BackupRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new backup in a background thread.
    pub fn start(&mut self, source: PathBuf, dest: PathBuf, max_versions: usize) {
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);

        self.progress = BackupProgress {
            start_time: Some(Local::now()),
            ..BackupProgress::default()
        };

        let cancel_flag = self.cancel_flag.clone();

        std::thread::spawn(move || {
            let opts = BackupOptions {
                cancel_flag,
                progress_tx: tx,
            };

            let result = copy::perform_backup(&source, &dest, opts);

            if result.is_ok() {
                let _ = copy::cleanup_old_versions(&dest, max_versions);
            }
        });
    }

    /// Signals cancellation to the running backup thread.
    pub fn cancel(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// Polls all pending progress events and updates internal state.
    /// Returns `true` when the backup has just completed (Finished event received).
    pub fn poll(&mut self) -> bool {
        let rx = match &self.receiver {
            Some(rx) => rx,
            None => return false,
        };

        let mut just_finished = false;

        for event in rx.try_iter() {
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
                    self.progress.current_index = index + 1; // 1-based for display
                }
                ProgressEvent::FileCompleted { bytes_copied } => {
                    self.progress.bytes_copied += bytes_copied;
                }
                ProgressEvent::Finished => {
                    self.progress.finished = true;
                    just_finished = true;
                }
            }
        }

        just_finished
    }

    /// Computes speed (bytes/sec) and ETA (seconds) for the current backup.
    /// Returns `None` if progress hasn't started, is finished, or no data yet.
    pub fn compute_eta(&self) -> Option<(f64, u64)> {
        if !self.progress.started || self.progress.finished {
            return None;
        }
        let start = self.progress.start_time?;
        let elapsed = (Local::now() - start).num_seconds().max(1) as f64;
        let bytes_copied = self.progress.bytes_copied;
        if bytes_copied == 0 {
            return None;
        }
        let speed = bytes_copied as f64 / elapsed;
        let remaining = (self.progress.total_bytes - bytes_copied) as f64;
        let eta = (remaining / speed) as u64;
        Some((speed, eta))
    }
}
