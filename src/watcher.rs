use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::index::{ChangeKind, ImageIndex};
use crate::scanner::scan_directory;
use crate::types::{is_image_extension, ws_add, ws_remove, ws_update, ImageEntry};

#[derive(Debug)]
struct PendingEvent {
    kind: EventKind,
    path: PathBuf,
    received_at: Instant,
}

pub fn start_watcher(
    root: PathBuf,
    index: Arc<ImageIndex>,
    tx: broadcast::Sender<String>,
    deny_set: HashSet<String>,
) -> notify::Result<()> {
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = event_tx.send(event);
            }
        },
        notify::Config::default(),
    )?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    let watcher = Arc::new(Mutex::new(watcher));

    tokio::spawn(process_events(
        event_rx,
        root,
        index,
        tx,
        deny_set,
        watcher,
    ));

    Ok(())
}

async fn process_events(
    mut event_rx: mpsc::UnboundedReceiver<Event>,
    root: PathBuf,
    index: Arc<ImageIndex>,
    tx: broadcast::Sender<String>,
    deny_set: HashSet<String>,
    watcher: Arc<Mutex<RecommendedWatcher>>,
) {
    let mut pending: HashMap<PathBuf, PendingEvent> = HashMap::new();
    let debounce_duration = Duration::from_millis(100);
    let tick_interval = Duration::from_millis(50);

    loop {
        // Drain incoming events with a timeout
        match tokio::time::timeout(tick_interval, event_rx.recv()).await {
            Ok(Some(event)) => {
                for path in event.paths {
                    pending.insert(
                        path.clone(),
                        PendingEvent {
                            kind: event.kind,
                            path,
                            received_at: Instant::now(),
                        },
                    );
                }
            }
            Ok(None) => break, // Channel closed
            Err(_) => {}       // Timeout — check pending events
        }

        // Process events that have aged past the debounce window
        let now = Instant::now();
        let ready: Vec<PendingEvent> = pending
            .iter()
            .filter(|(_, pe)| now.duration_since(pe.received_at) >= debounce_duration)
            .map(|(_, pe)| PendingEvent {
                kind: pe.kind,
                path: pe.path.clone(),
                received_at: pe.received_at,
            })
            .collect();

        for pe in ready {
            pending.remove(&pe.path);
            dispatch_event(
                &pe.kind,
                &pe.path,
                &root,
                &index,
                &tx,
                &deny_set,
                &watcher,
            )
            .await;
        }
    }
}

async fn dispatch_event(
    kind: &EventKind,
    path: &Path,
    root: &Path,
    index: &Arc<ImageIndex>,
    tx: &broadcast::Sender<String>,
    deny_set: &HashSet<String>,
    watcher: &Arc<Mutex<RecommendedWatcher>>,
) {
    match kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            if path.is_dir() {
                handle_new_directory(path, root, index, tx, deny_set, watcher).await;
            } else if is_image_extension(path) {
                handle_create_or_modify(path, root, index, tx).await;
            }
        }
        EventKind::Remove(_) => {
            if is_image_extension(path) {
                handle_remove_file(path, root, index, tx).await;
            } else {
                // Could be a directory removal
                let rel = match path.strip_prefix(root) {
                    Ok(r) => r.to_path_buf(),
                    Err(_) => return,
                };
                let removed = index.remove_under(&rel).await;
                for p in removed {
                    let _ = tx.send(ws_remove(&p));
                }
            }
        }
        EventKind::Access(_) | EventKind::Other | EventKind::Any => {}
    }
}

async fn handle_create_or_modify(
    path: &Path,
    root: &Path,
    index: &Arc<ImageIndex>,
    tx: &broadcast::Sender<String>,
) {
    let rel = match path.strip_prefix(root) {
        Ok(r) => r.to_path_buf(),
        Err(_) => return,
    };

    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };

    let entry = ImageEntry {
        path: rel,
        mtime: metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        size: metadata.len(),
    };

    let (change, entry) = index.add_or_update(entry).await;
    match change {
        ChangeKind::Added => {
            let _ = tx.send(ws_add(&entry));
        }
        ChangeKind::Updated => {
            let _ = tx.send(ws_update(&entry));
        }
        ChangeKind::Unchanged => {}
    }
}

async fn handle_remove_file(
    path: &Path,
    root: &Path,
    index: &Arc<ImageIndex>,
    tx: &broadcast::Sender<String>,
) {
    let rel = match path.strip_prefix(root) {
        Ok(r) => r.to_path_buf(),
        Err(_) => return,
    };

    if index.remove(&rel).await.is_some() {
        let _ = tx.send(ws_remove(&rel));
    }
}

async fn handle_new_directory(
    path: &Path,
    root: &Path,
    index: &Arc<ImageIndex>,
    tx: &broadcast::Sender<String>,
    deny_set: &HashSet<String>,
    watcher: &Arc<Mutex<RecommendedWatcher>>,
) {
    // Check deny list
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if deny_set.contains(name) {
            return;
        }
    }

    // Watch first, scan second (spec §2.3 race condition mitigation)
    {
        let mut w = watcher.lock().await;
        if let Err(e) = w.watch(path, RecursiveMode::Recursive) {
            tracing::warn!("Failed to watch new directory {}: {}", path.display(), e);
        }
    }

    // Scan the directory contents
    let entries = tokio::task::spawn_blocking({
        let path = path.to_path_buf();
        let root = root.to_path_buf();
        let deny_set = deny_set.clone();
        move || scan_directory(&path, &root, &deny_set)
    })
    .await
    .unwrap_or_default();

    for entry in entries {
        let (change, entry) = index.add_or_update(entry).await;
        match change {
            ChangeKind::Added => {
                let _ = tx.send(ws_add(&entry));
            }
            ChangeKind::Updated => {
                let _ = tx.send(ws_update(&entry));
            }
            ChangeKind::Unchanged => {}
        }
    }
}
