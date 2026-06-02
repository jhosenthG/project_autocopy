use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::copy::{self, BackupOptions, ProgressEvent};
use crate::error::BackupResult;

pub struct BackupOrchestrator {
    cancel_flag: Arc<AtomicBool>,
}

impl BackupOrchestrator {
    pub fn new() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn run(
        &self,
        source: &Path,
        dest: &Path,
        max_versions: usize,
        progress_tx: Sender<ProgressEvent>,
    ) -> BackupResult<PathBuf> {
        self.cancel_flag.store(false, Ordering::Relaxed);

        let opts = BackupOptions {
            cancel_flag: self.cancel_flag.clone(),
            progress_tx,
        };

        let result = copy::perform_backup(source, dest, opts);

        match result {
            Ok(backup_path) => {
                let _ = copy::cleanup_old_versions(dest, max_versions);
                Ok(backup_path)
            }
            Err(e) => Err(e),
        }
    }
}

impl Default for BackupOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}
