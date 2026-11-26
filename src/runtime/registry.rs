use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

/// Resource categories backed by real loaders and managers in Egotop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    ApplicationConfig,
    WindowConfig,
    Theme,
    Animation,
    Background,
    Layout,
    Font,
}

impl ResourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::ApplicationConfig => "ApplicationConfig",
            Self::WindowConfig => "WindowConfig",
            Self::Theme => "Theme",
            Self::Animation => "Animation",
            Self::Background => "Background",
            Self::Layout => "Layout",
            Self::Font => "Font",
        }
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Typed identifier avoiding arbitrary untyped strings when tracking resources.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId {
    pub kind: ResourceKind,
    pub key: String,
}

impl ResourceId {
    pub fn new(kind: ResourceKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into().trim().to_ascii_lowercase(),
        }
    }

    pub fn app_config() -> Self {
        Self::new(ResourceKind::ApplicationConfig, "egotop")
    }

    pub fn window_config() -> Self {
        Self::new(ResourceKind::WindowConfig, "window")
    }

    pub fn background() -> Self {
        Self::new(ResourceKind::Background, "active")
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.label(), self.key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    Unloaded,
    Loaded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    Pending,
    Valid,
    Invalid,
}

/// Tracked metadata for a single reloadable configuration resource.
#[derive(Debug, Clone)]
pub struct ResourceRecord {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub source_path: PathBuf,
    pub version: u64,
    pub load_status: LoadStatus,
    pub validation_status: ValidationStatus,
    pub dependencies: Vec<ResourceId>,
}

/// Central catalog of reloadable resources, versions, statuses, and dependencies.
#[derive(Debug, Clone, Default)]
pub struct ResourceRegistry {
    records: BTreeMap<ResourceId, ResourceRecord>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(
        &mut self,
        id: ResourceId,
        source_path: PathBuf,
        load_status: LoadStatus,
        validation_status: ValidationStatus,
        dependencies: Vec<ResourceId>,
    ) -> u64 {
        let next_version = self
            .records
            .get(&id)
            .map(|r| r.version.saturating_add(1))
            .unwrap_or(1);
        let kind = id.kind;
        self.records.insert(
            id.clone(),
            ResourceRecord {
                id,
                kind,
                source_path,
                version: next_version,
                load_status,
                validation_status,
                dependencies,
            },
        );
        next_version
    }

    pub fn update_status(
        &mut self,
        id: &ResourceId,
        load_status: LoadStatus,
        validation_status: ValidationStatus,
    ) {
        if let Some(rec) = self.records.get_mut(id) {
            rec.version = rec.version.saturating_add(1);
            rec.load_status = load_status;
            rec.validation_status = validation_status;
        }
    }

    pub fn remove_by_path(&mut self, path: &std::path::Path) -> Vec<ResourceId> {
        let matches: Vec<ResourceId> = self
            .records
            .iter()
            .filter(|(_, r)| r.source_path == path)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &matches {
            self.records.remove(id);
        }
        matches
    }

    pub fn find_by_path(&self, path: &std::path::Path) -> Vec<&ResourceRecord> {
        self.records
            .values()
            .filter(|r| r.source_path == path)
            .collect()
    }

    pub fn get(&self, id: &ResourceId) -> Option<&ResourceRecord> {
        self.records.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &ResourceRecord> {
        self.records.values()
    }

    pub fn by_kind(&self, kind: ResourceKind) -> Vec<&ResourceRecord> {
        self.records.values().filter(|r| r.kind == kind).collect()
    }

    /// Returns all registered resources that depend directly or transitively on `target`.
    pub fn dependents_of(&self, target: &ResourceId) -> Vec<ResourceId> {
        let mut visited = BTreeSet::new();
        let mut queue = vec![target.clone()];
        let mut out = Vec::new();

        while let Some(cur) = queue.pop() {
            for rec in self.records.values() {
                if rec.dependencies.contains(&cur) && visited.insert(rec.id.clone()) {
                    out.push(rec.id.clone());
                    queue.push(rec.id.clone());
                }
            }
        }
        out
    }

    /// Detects whether registering `dependencies` for `root` introduces a dependency cycle.
    pub fn detect_cycle(&self, root: &ResourceId, candidate_deps: &[ResourceId]) -> bool {
        let mut visited = BTreeSet::new();
        let mut stack: Vec<ResourceId> = candidate_deps.to_vec();
        while let Some(node) = stack.pop() {
            if &node == root {
                return true;
            }
            if !visited.insert(node.clone()) {
                continue;
            }
            if let Some(rec) = self.records.get(&node) {
                for dep in &rec.dependencies {
                    stack.push(dep.clone());
                }
            }
        }
        false
    }

    /// Checks whether all declared dependencies exist in the registry (or built-in pool).
    pub fn contains(&self, id: &ResourceId) -> bool {
        self.records.contains_key(id)
    }
}
