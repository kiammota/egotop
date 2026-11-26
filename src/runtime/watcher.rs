use crate::runtime::debouncer::WatchEventKind;
use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct FileWatchEvent {
    pub path: PathBuf,
    pub kind: WatchEventKind,
}

/// Non-blocking filesystem watcher wrapped cleanly around `notify::RecommendedWatcher`.
pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    tx: Sender<FileWatchEvent>,
    rx: Receiver<FileWatchEvent>,
    error_rx: Receiver<String>,
    watched_roots: Vec<PathBuf>,
    last_error: Option<String>,
}

impl FileWatcher {
    /// Starts watching `roots` (deduplicated) without blocking the caller.
    pub fn start(roots: &[PathBuf]) -> Self {
        let (tx, rx) = mpsc::channel();
        let (err_tx, error_rx) = mpsc::channel();
        let mut instance = Self {
            watcher: None,
            tx,
            rx,
            error_rx,
            watched_roots: Vec::new(),
            last_error: None,
        };
        instance.init_watcher(roots, &err_tx);
        instance
    }

    /// Creates an inert in-memory watcher for deterministic tests without OS watches.
    pub fn offline() -> Self {
        let (tx, rx) = mpsc::channel();
        let (_err_tx, error_rx) = mpsc::channel();
        Self {
            watcher: None,
            tx,
            rx,
            error_rx,
            watched_roots: Vec::new(),
            last_error: None,
        }
    }

    fn init_watcher(&mut self, roots: &[PathBuf], err_tx: &Sender<String>) {
        let tx_clone = self.tx.clone();
        let err_clone = err_tx.clone();
        let watcher_res = notify::recommended_watcher(move |res: notify::Result<Event>| match res {
            Ok(ev) => {
                for normalized in translate_notify_event(ev) {
                    let _ = tx_clone.send(normalized);
                }
            }
            Err(err) => {
                let _ = err_clone.send(err.to_string());
                let _ = tx_clone.send(FileWatchEvent {
                    path: PathBuf::new(),
                    kind: WatchEventKind::Rescan,
                });
            }
        });

        match watcher_res {
            Ok(mut w) => {
                let mut deduped = Vec::new();
                for root in roots {
                    if !root.exists() {
                        continue;
                    }
                    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
                    if deduped
                        .iter()
                        .any(|existing: &PathBuf| canonical.starts_with(existing))
                    {
                        continue;
                    }
                    if w.watch(&canonical, RecursiveMode::Recursive).is_ok() {
                        deduped.push(canonical);
                    }
                }
                self.watched_roots = deduped;
                self.watcher = Some(w);
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(err.to_string());
            }
        }
    }

    /// Ensures newly created configuration directories inside `roots` are watched.
    pub fn ensure_roots_watched(&mut self, roots: &[PathBuf]) {
        let Some(w) = &mut self.watcher else {
            return;
        };
        for root in roots {
            if !root.exists() {
                continue;
            }
            let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
            if self
                .watched_roots
                .iter()
                .any(|existing| canonical.starts_with(existing))
            {
                continue;
            }
            if w.watch(&canonical, RecursiveMode::Recursive).is_ok() {
                self.watched_roots.push(canonical);
            }
        }
    }

    /// Drains all queued filesystem events immediately without blocking.
    pub fn drain_events(&mut self) -> (Vec<FileWatchEvent>, Vec<String>) {
        let mut events = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            events.push(ev);
        }
        let mut errors = Vec::new();
        while let Ok(err) = self.error_rx.try_recv() {
            self.last_error = Some(err.clone());
            errors.push(err);
        }
        (events, errors)
    }

    /// Injects a synthetic event cleanly into the same channel (used by tests and recovery).
    pub fn inject_event(&self, path: PathBuf, kind: WatchEventKind) {
        let _ = self.tx.send(FileWatchEvent { path, kind });
    }

    /// Re-initializes the underlying OS watcher if an error occurred or watches dropped.
    pub fn recover(&mut self, roots: &[PathBuf]) -> bool {
        let (err_tx, error_rx) = mpsc::channel();
        self.error_rx = error_rx;
        self.watched_roots.clear();
        self.watcher = None;
        self.init_watcher(roots, &err_tx);
        self.watcher.is_some()
    }

    pub fn watched_roots(&self) -> &[PathBuf] {
        &self.watched_roots
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

fn translate_notify_event(ev: Event) -> Vec<FileWatchEvent> {
    if ev.need_rescan() {
        return vec![FileWatchEvent {
            path: PathBuf::new(),
            kind: WatchEventKind::Rescan,
        }];
    }

    let kind = match ev.kind {
        EventKind::Create(CreateKind::File | CreateKind::Folder | CreateKind::Any | CreateKind::Other) => {
            WatchEventKind::Created
        }
        EventKind::Modify(ModifyKind::Name(_)) => WatchEventKind::Renamed,
        EventKind::Modify(
            ModifyKind::Data(_) | ModifyKind::Metadata(_) | ModifyKind::Any | ModifyKind::Other,
        ) => WatchEventKind::Modified,
        EventKind::Remove(RemoveKind::File | RemoveKind::Folder | RemoveKind::Any | RemoveKind::Other) => {
            WatchEventKind::Removed
        }
        // Ignore pure read/open events to prevent spurious reload loops
        EventKind::Access(_) => return Vec::new(),
        EventKind::Other | EventKind::Any => WatchEventKind::Modified,
    };

    ev.paths
        .into_iter()
        .filter(|p| !is_ignored_editor_artifact(p))
        .map(|path| FileWatchEvent { path, kind })
        .collect()
}

fn is_ignored_editor_artifact(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };
    name.starts_with(".#")
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".swx")
        || name.ends_with(".tmp")
        || name == "4913" // Vim/Neovim test probe file during atomic save checks
}
