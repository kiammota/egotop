pub mod coordinator;
pub mod debouncer;
pub mod diagnostics;
pub mod managers;
pub mod pipeline;
pub mod registry;
pub mod state;
pub mod watcher;

pub use coordinator::{ConfigurationRuntime, RuntimePaths};
pub use debouncer::{ChangeDebouncer, PendingChange, WatchEventKind};
pub use diagnostics::{
    Diagnostic, DiagnosticEngine, DiagnosticSeverity, RuntimeLogEntry, line_col_from_offset,
};
pub use managers::{ManagedAppTarget, RuntimeResourceManager};
pub use pipeline::{
    LayoutPreset, ParsedPayload, ParsedResource, ValidationContext, parse_resource,
    validate_schema, validate_semantics,
};
pub use registry::{
    LoadStatus, ResourceId, ResourceKind, ResourceRecord, ResourceRegistry, ValidationStatus,
};
pub use state::{LastKnownGoodState, RuntimeCommit};
pub use watcher::{FileWatchEvent, FileWatcher};
