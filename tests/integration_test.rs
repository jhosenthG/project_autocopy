use autocopy::copy::{self, BackupOptions};
use autocopy::error::BackupError;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use tempfile::TempDir;

fn create_backup_opts() -> BackupOptions {
    BackupOptions {
        cancel_flag: Arc::new(AtomicBool::new(false)),
        progress_tx: mpsc::channel().0,
    }
}

#[test]
fn test_full_backup_flow() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();

    fs::write(source_dir.path().join("test.txt"), "hello world").unwrap();
    fs::create_dir(source_dir.path().join("subdir")).unwrap();
    fs::write(source_dir.path().join("subdir").join("file.txt"), "content").unwrap();

    let result = copy::perform_backup(source_dir.path(), dest_dir.path(), create_backup_opts());

    assert!(result.is_ok());
    let backup_path = result.unwrap();
    assert!(backup_path.exists());
    assert!(backup_path.join("test.txt").exists());
    assert!(backup_path.join("subdir").join("file.txt").exists());
}

#[test]
fn test_cleanup_after_backup() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();

    fs::write(source_dir.path().join("file.txt"), "content").unwrap();

    for i in 0..5 {
        let timestamp = format!("2024-01-{:02}_10-00-00", i + 1);
        fs::create_dir(dest_dir.path().join(format!("backup_{}", timestamp))).unwrap();
    }

    let _ = copy::perform_backup(source_dir.path(), dest_dir.path(), create_backup_opts());
    copy::cleanup_old_versions(dest_dir.path(), 3).unwrap();

    let versions = copy::list_versions(dest_dir.path()).unwrap();
    assert_eq!(versions.len(), 3);
}

#[test]
fn test_cleanup_removes_old_versions() {
    let temp_dir = TempDir::new().unwrap();
    let dest = temp_dir.path();

    for i in 0..5 {
        let folder = format!("backup_2024-01-0{}_10-00-00", i + 1);
        fs::create_dir(dest.join(folder)).unwrap();
    }

    let deleted = copy::cleanup_old_versions(dest, 3).unwrap();
    assert_eq!(deleted, 2);

    let versions = copy::list_versions(dest).unwrap();
    assert_eq!(versions.len(), 3);
}

#[test]
fn test_validate_paths_rejects_same_folder() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    let result = copy::validate_paths(path, path);
    assert!(result.is_err());

    match result {
        Err(BackupError::SameFolder) => (),
        _ => panic!("Expected SameFolder error"),
    }
}

#[test]
fn test_validate_paths_rejects_nonexistent_source() {
    let temp_dir = TempDir::new().unwrap();

    let result = copy::validate_paths(Path::new("C:/nonexistent_path_12345"), temp_dir.path());
    assert!(result.is_err());
}
