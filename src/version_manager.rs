use std::path::{Path, PathBuf};

use crate::copy;

/// Sort order for the version list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortOrder {
    Newest,
    Oldest,
    Largest,
}

/// Manages querying, sorting, filtering, and deleting backup versions.
///
/// Keeps version list logic out of the UI layer so it can be tested
/// and changed independently.
pub struct VersionManager {
    pub versions: Vec<PathBuf>,
    pub sort_order: SortOrder,
    pub filter: String,
    dest: Option<PathBuf>,
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
            sort_order: SortOrder::Newest,
            filter: String::new(),
            dest: None,
        }
    }

    /// Sets the destination directory to scan for backup versions.
    pub fn set_dest(&mut self, dest: Option<PathBuf>) {
        self.dest = dest;
    }

    /// Re-reads versions from the destination folder, applying current sort and filter.
    pub fn refresh(&mut self) {
        let dest = match &self.dest {
            Some(d) => d,
            None => {
                self.versions.clear();
                return;
            }
        };

        let mut versions = copy::list_versions(dest).unwrap_or_default();

        match self.sort_order {
            SortOrder::Newest => versions.sort_by(|a, b| b.cmp(a)),
            SortOrder::Oldest => versions.sort(),
            SortOrder::Largest => versions.sort_by(|a, b| {
                let size_a = folder_size(a);
                let size_b = folder_size(b);
                size_b.cmp(&size_a)
            }),
        }

        if !self.filter.is_empty() {
            let filter = self.filter.to_lowercase();
            versions.retain(|v| {
                v.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase().contains(&filter))
                    .unwrap_or(false)
            });
        }

        self.versions = versions;
    }

    /// Permanently deletes a version folder and refreshes the list.
    pub fn delete_version(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(path)?;
        self.refresh();
        Ok(())
    }
}

/// Calculate the total size of a directory by walking its contents.
pub fn folder_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total += meta.len();
            }
        }
    }
    total
}
