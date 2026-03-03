use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub path: PathBuf,
    pub mtime: SystemTime,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct ImageInfo {
    pub path: String,
    pub url: String,
    pub mtime: u64,
    pub size: u64,
}

impl ImageEntry {
    pub fn to_info(&self) -> ImageInfo {
        let path_str = self.path.to_string_lossy().to_string();
        let mtime = self
            .mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        ImageInfo {
            url: format!("/image/{}", path_str),
            path: path_str,
            mtime,
            size: self.size,
        }
    }
}

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tiff", "tif", "ico", "avif",
];

pub const DEFAULT_DENY_LIST: &[&str] = &[
    "node_modules",
    ".git",
    ".hg",
    ".svn",
    ".cache",
    ".venv",
    ".env",
    "__pycache__",
    "target",
    "dist",
    "build",
    "vendor",
    ".next",
    ".nuxt",
    ".output",
    ".turbo",
    ".parcel-cache",
    "coverage",
    "tmp",
    "temp",
];

pub fn is_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            let lower = ext.to_ascii_lowercase();
            SUPPORTED_EXTENSIONS.iter().any(|&s| s == lower)
        })
}

pub fn content_type_for_extension(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

pub fn ws_add(entry: &ImageEntry) -> String {
    let info = entry.to_info();
    serde_json::json!({"type": "add", "image": info}).to_string()
}

pub fn ws_update(entry: &ImageEntry) -> String {
    let info = entry.to_info();
    serde_json::json!({"type": "update", "image": info}).to_string()
}

pub fn ws_remove(path: &Path) -> String {
    serde_json::json!({"type": "remove", "path": path.to_string_lossy()}).to_string()
}
