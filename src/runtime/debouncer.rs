use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Normalized event kind delivered from `FileWatcher` into `ChangeDebouncer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Modified,
    Created,
    Removed,
    Renamed,
    Rescan,
}

#[derive(Debug, Clone)]
pub struct PendingChange {
    pub path: PathBuf,
    pub kind: WatchEventKind,
    pub last_event_at: Instant,
    pub event_count: usize,
}

/// Central debouncer coalescing rapid bursts (e.g. atomic editor saves) per path.
#[derive(Debug, Clone)]
pub struct ChangeDebouncer {
    window: Duration,
    pending: BTreeMap<PathBuf, PendingChange>,
    rescan_pending_since: Option<Instant>,
}

impl Default for ChangeDebouncer {
    fn default() -> Self {
        Self::new(Duration::from_millis(120))
    }
}

impl ChangeDebouncer {
    pub fn new(window: Duration) -> Self {
        Self {
            window: window.max(Duration::from_millis(10)),
            pending: BTreeMap::new(),
            rescan_pending_since: None,
        }
    }

    pub fn window(&self) -> Duration {
        self.window
    }

    pub fn set_window(&mut self, window: Duration) {
        self.window = window.max(Duration::from_millis(10));
    }

    pub fn push(&mut self, path: PathBuf, kind: WatchEventKind, now: Instant) {
        if kind == WatchEventKind::Rescan {
            self.rescan_pending_since = Some(now);
            return;
        }
        match self.pending.get_mut(&path) {
            Some(entry) => {
                entry.kind = merge_kind(entry.kind, kind);
                entry.last_event_at = now;
                entry.event_count += 1;
            }
            None => {
                self.pending.insert(
                    path.clone(),
                    PendingChange {
                        path,
                        kind,
                        last_event_at: now,
                        event_count: 1,
                    },
                );
            }
        }
    }

    /// Returns `(ready_changes, full_rescan_triggered)` for entries whose debounce window elapsed.
    pub fn flush_ready(&mut self, now: Instant) -> (Vec<PendingChange>, bool) {
        let rescan = self
            .rescan_pending_since
            .is_some_and(|t| now.duration_since(t) >= self.window);
        if rescan {
            self.rescan_pending_since = None;
            self.pending.clear();
            return (Vec::new(), true);
        }

        let ready_keys: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_event_at) >= self.window)
            .map(|(p, _)| p.clone())
            .collect();

        let mut ready = Vec::with_capacity(ready_keys.len());
        for key in ready_keys {
            if let Some(item) = self.pending.remove(&key) {
                ready.push(item);
            }
        }
        (ready, false)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.rescan_pending_since.is_none()
    }
}

fn merge_kind(prev: WatchEventKind, next: WatchEventKind) -> WatchEventKind {
    match (prev, next) {
        (_, WatchEventKind::Removed) => WatchEventKind::Removed,
        (WatchEventKind::Removed, WatchEventKind::Created | WatchEventKind::Modified) => {
            // Editor atomic save often unlinks then recreates/moves the file
            WatchEventKind::Modified
        }
        (_, WatchEventKind::Renamed) => WatchEventKind::Renamed,
        _ => next,
    }
}
