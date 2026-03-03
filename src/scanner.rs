use std::collections::HashSet;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::types::{is_image_extension, ImageEntry, DEFAULT_DENY_LIST};

pub fn build_deny_set(user_extra: &[String]) -> HashSet<String> {
    let mut set: HashSet<String> = DEFAULT_DENY_LIST.iter().map(|s| s.to_string()).collect();
    for entry in user_extra {
        for part in entry.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                set.insert(trimmed.to_string());
            }
        }
    }
    set
}

/// Scan a directory recursively, returning image entries with paths relative to `root`.
/// Shared by initial scan and new-directory events (spec §1.1).
pub fn scan_directory(scan_path: &Path, root: &Path, deny_set: &HashSet<String>) -> Vec<ImageEntry> {
    let mut entries = Vec::new();

    let walker = WalkDir::new(scan_path).follow_links(false).into_iter();

    for entry in walker.filter_entry(|e| {
        if e.file_type().is_dir() {
            let name = e.file_name().to_string_lossy();
            !deny_set.contains(name.as_ref())
        } else {
            true
        }
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!("Error scanning directory entry: {}", err);
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if !is_image_extension(path) {
            continue;
        }

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(err) => {
                tracing::warn!("Cannot read metadata for {}: {}", path.display(), err);
                continue;
            }
        };

        let mtime = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let size = metadata.len();

        let rel_path = match path.strip_prefix(root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => {
                tracing::warn!("Path {} is not under root {}", path.display(), root.display());
                continue;
            }
        };

        entries.push(ImageEntry {
            path: rel_path,
            mtime,
            size,
        });
    }

    // Sort by mtime descending (newest first)
    entries.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    entries
}
