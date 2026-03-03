use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

use crate::types::ImageEntry;

#[derive(Debug)]
pub enum ChangeKind {
    Added,
    Updated,
    Unchanged,
}

pub struct ImageIndex {
    entries: RwLock<Vec<ImageEntry>>,
}

impl ImageIndex {
    pub fn new(mut entries: Vec<ImageEntry>) -> Self {
        entries.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        Self {
            entries: RwLock::new(entries),
        }
    }

    /// Combined add-or-update in a single write lock (spec §2.4).
    /// Returns the change kind and the entry if something changed.
    pub async fn add_or_update(&self, entry: ImageEntry) -> (ChangeKind, ImageEntry) {
        let mut entries = self.entries.write().await;
        if let Some(pos) = entries.iter().position(|e| e.path == entry.path) {
            if entries[pos].mtime == entry.mtime && entries[pos].size == entry.size {
                return (ChangeKind::Unchanged, entry);
            }
            // Update: remove old, insert at front (newest)
            entries.remove(pos);
            entries.insert(0, entry.clone());
            (ChangeKind::Updated, entry)
        } else {
            // Add: insert at front (newest)
            entries.insert(0, entry.clone());
            (ChangeKind::Added, entry)
        }
    }

    pub async fn remove(&self, path: &Path) -> Option<ImageEntry> {
        let mut entries = self.entries.write().await;
        if let Some(pos) = entries.iter().position(|e| e.path == path) {
            Some(entries.remove(pos))
        } else {
            None
        }
    }

    /// Remove all images whose path starts with the given directory prefix.
    pub async fn remove_under(&self, dir: &Path) -> Vec<PathBuf> {
        let mut entries = self.entries.write().await;
        let mut removed = Vec::new();
        entries.retain(|e| {
            if e.path.starts_with(dir) {
                removed.push(e.path.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub async fn get_all(&self) -> Vec<ImageEntry> {
        self.entries.read().await.clone()
    }
}
