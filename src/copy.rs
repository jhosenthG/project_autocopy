use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use walkdir::WalkDir;

use crate::error::{BackupError, BackupResult};
use chrono::Local;
use sysinfo::Disks;

const SKIP_FILES: &[&str] = &["Thumbs.db", "desktop.ini", ".DS_Store", "Icon\r"];
const SKIP_EXTENSIONS: &[&str] = &[];

pub struct BackupOptions {
    pub cancel_flag: Arc<AtomicBool>,
    pub progress_tx: Sender<ProgressEvent>,
}

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Started {
        total_files: usize,
        total_bytes: u64,
    },
    FileStarted {
        path: PathBuf,
        index: usize,
    },
    FileCompleted {
        bytes_copied: u64,
    },
    Finished,
}

pub fn perform_backup(source: &Path, dest: &Path, opts: BackupOptions) -> BackupResult<PathBuf> {
    validate_paths(source, dest)?;

    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let backup_name = format!("backup_{}", timestamp);
    let tmp_backup_path = dest.join(format!("{}.tmp", backup_name));

    if opts.cancel_flag.load(Ordering::Relaxed) {
        return Err(BackupError::Cancelled);
    }

    let mut total_bytes: u64 = 0;
    let entries: Vec<_> = WalkDir::new(source)
        .into_iter()
        .filter_entry(|e| !is_skip_entry(e.path()))
        .filter_map(|e| e.ok())
        .filter(|e| {
            if e.file_type().is_file() {
                if let Ok(meta) = e.metadata() {
                    total_bytes += meta.len();
                }
                true
            } else {
                false
            }
        })
        .collect();

    let total_files = entries.len();

    let _ = opts.progress_tx.send(ProgressEvent::Started {
        total_files,
        total_bytes,
    });

    fs::create_dir_all(&tmp_backup_path).map_err(BackupError::CreateDirFailed)?;

    for (index, entry) in entries.iter().enumerate() {
        if opts.cancel_flag.load(Ordering::Relaxed) {
            let _ = fs::remove_dir_all(&tmp_backup_path);
            return Err(BackupError::Cancelled);
        }

        let relative_path =
            entry
                .path()
                .strip_prefix(source)
                .map_err(|_| BackupError::CopyFailed {
                    from: entry.path().to_path_buf(),
                    to: tmp_backup_path.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Failed to compute relative path",
                    ),
                })?;

        let target_path = tmp_backup_path.join(relative_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let _ = opts.progress_tx.send(ProgressEvent::FileStarted {
            path: relative_path.to_path_buf(),
            index,
        });

        let bytes = match fs::metadata(entry.path()) {
            Ok(m) => m.len(),
            Err(_) => 0,
        };

        copy_file(entry.path(), &target_path)?;

        let _ = opts.progress_tx.send(ProgressEvent::FileCompleted {
            bytes_copied: bytes,
        });
    }

    let final_path = dest.join(&backup_name);
    fs::rename(&tmp_backup_path, &final_path).map_err(|e| {
        fs::remove_dir_all(&tmp_backup_path).ok();
        BackupError::CreateDirFailed(e)
    })?;

    let _ = opts.progress_tx.send(ProgressEvent::Finished);

    Ok(final_path)
}

fn copy_file(from: &Path, to: &Path) -> BackupResult<()> {
    fs::copy(from, to).map_err(|e| BackupError::CopyFailed {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

fn is_skip_entry(path: &Path) -> bool {
    if let Some(name) = path.file_name() {
        let name_str = name.to_string_lossy();
        if SKIP_FILES.iter().any(|&s| name_str.eq_ignore_ascii_case(s)) {
            return true;
        }
    }

    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        if SKIP_EXTENSIONS
            .iter()
            .any(|&e| ext_str.eq_ignore_ascii_case(e))
        {
            return true;
        }
    }

    false
}

pub fn list_versions(dest: &Path) -> BackupResult<Vec<PathBuf>> {
    if !dest.exists() {
        return Ok(Vec::new());
    }

    let mut versions: Vec<PathBuf> = fs::read_dir(dest)
        .map_err(BackupError::CreateDirFailed)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return false;
            }
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                return name_str.starts_with("backup_") && !name_str.ends_with(".tmp");
            }
            false
        })
        .map(|entry| entry.path())
        .collect();

    versions.sort_by(|a, b| b.cmp(a));
    Ok(versions)
}

pub fn cleanup_old_versions(dest: &Path, keep: usize) -> BackupResult<usize> {
    let versions = list_versions(dest)?;

    if versions.len() <= keep {
        return Ok(0);
    }

    let to_delete = &versions[keep..];
    let mut deleted = 0;

    for path in to_delete {
        match fs::remove_dir_all(path) {
            Ok(_) => deleted += 1,
            Err(e) => {
                eprintln!("Warning: failed to delete {}: {}", path.display(), e);
            }
        }
    }

    Ok(deleted)
}

pub fn validate_paths(source: &Path, dest: &Path) -> BackupResult<()> {
    if !source.exists() {
        return Err(BackupError::SourceNotFound(source.to_path_buf()));
    }

    if source == dest || source.to_string_lossy() == dest.to_string_lossy() {
        return Err(BackupError::SameFolder);
    }

    let source_canonical = source
        .canonicalize()
        .map_err(|_| BackupError::SourceNotFound(source.to_path_buf()))?;
    let dest_canonical = dest.canonicalize().map_err(|_| {
        BackupError::CreateDirFailed(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Destination not found",
        ))
    })?;

    if source_canonical == dest_canonical {
        return Err(BackupError::SameFolder);
    }

    if !dest.exists() {
        fs::create_dir_all(dest).map_err(BackupError::CreateDirFailed)?;
    }

    let estimated_size = estimate_size(source)?;
    let available_space = get_available_space(dest)?;

    let available_mb = available_space / (1024 * 1024);
    let needed_mb = estimated_size / (1024 * 1024);

    if estimated_size > available_space {
        return Err(BackupError::InsufficientSpace {
            needed_mb,
            available_mb,
        });
    }

    Ok(())
}

pub fn estimate_size(source: &Path) -> BackupResult<u64> {
    let mut total: u64 = 0;

    for entry in WalkDir::new(source)
        .into_iter()
        .filter_entry(|e| !is_skip_entry(e.path()))
        .filter_map(|e| e.ok())
    {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() {
                total += metadata.len();
            }
        }
    }

    Ok(total)
}

pub(crate) fn get_available_space(path: &Path) -> BackupResult<u64> {
    let disks = Disks::new_with_refreshed_list();
    let path_str = path.to_string_lossy();

    for disk in disks.iter() {
        let mount_point = disk.mount_point().to_string_lossy();
        if path_str.starts_with(&*mount_point) || mount_point == "/" {
            return Ok(disk.available_space());
        }
    }

    Ok(u64::MAX / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_list_versions_ignores_non_matching() {
        let temp_dir = TempDir::new().unwrap();
        let dest = temp_dir.path();

        fs::create_dir(dest.join("backup_2024-01-01_10-00-00")).unwrap();
        fs::create_dir(dest.join("other_folder")).unwrap();
        fs::create_dir(dest.join("backup_2024-01-02_10-00-00")).unwrap();
        fs::create_dir(dest.join("not_a_backup")).unwrap();

        let versions = list_versions(dest).unwrap();

        assert_eq!(versions.len(), 2);
        assert!(versions[0].to_string_lossy().contains("2024-01-02"));
        assert!(versions[1].to_string_lossy().contains("2024-01-01"));
    }

    #[test]
    fn test_cleanup_old_versions_keeps_only_n() {
        let temp_dir = TempDir::new().unwrap();
        let dest = temp_dir.path();

        for i in 0..5 {
            let folder = format!("backup_2024-01-0{}_10-00-00", i + 1);
            fs::create_dir(dest.join(folder)).unwrap();
        }

        let deleted = cleanup_old_versions(dest, 3).unwrap();

        assert_eq!(deleted, 2);

        let versions = list_versions(dest).unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn test_validate_paths_detects_same_folder() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        let result = validate_paths(path, path);
        assert!(matches!(result, Err(BackupError::SameFolder)));
    }

    #[test]
    fn test_skip_files() {
        assert!(is_skip_entry(Path::new("C:/Thumbs.db")));
        assert!(is_skip_entry(Path::new("D:/folder/desktop.ini")));
        assert!(!is_skip_entry(Path::new("D:/folder/document.txt")));
    }
}
