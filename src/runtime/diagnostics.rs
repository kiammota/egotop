use crate::runtime::registry::ResourceId;
use chrono::Local;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Diagnostic severity level exposed cleanly to TUI panels, commands, and logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

impl DiagnosticSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// Structured, UI-agnostic configuration diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub field_path: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub suggestion: Option<String>,
    pub timestamp: String,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Error, code, message)
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Warning, code, message)
    }

    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DiagnosticSeverity::Info, code, message)
    }

    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            file: None,
            line: None,
            column: None,
            field_path: None,
            expected: None,
            actual: None,
            suggestion: None,
            timestamp: Local::now().format("%H:%M:%S").to_string(),
        }
    }

    pub fn with_location(
        mut self,
        file: Option<&Path>,
        line: Option<usize>,
        column: Option<usize>,
    ) -> Self {
        self.file = file.map(Path::to_path_buf);
        self.line = line;
        self.column = column;
        self
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field_path = Some(field.into());
        self
    }

    pub fn with_expected_actual(
        mut self,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Renders a multi-line structured diagnostic block matching Egotop's diagnostic format.
    pub fn format_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("{} {}", self.severity.label(), self.code)];
        if let Some(file) = &self.file {
            lines.push(format!("File: {}", file.display()));
        }
        if let Some(line) = self.line {
            lines.push(format!("Line: {line}"));
        }
        if let Some(col) = self.column {
            lines.push(format!("Column: {col}"));
        }
        if let Some(field) = &self.field_path {
            lines.push(format!("Field: {field}"));
        }
        lines.push(self.message.clone());
        if let Some(expected) = &self.expected {
            lines.push(format!("Expected: {expected}"));
        }
        if let Some(actual) = &self.actual {
            lines.push(format!("Actual: {actual}"));
        }
        if let Some(hint) = &self.suggestion {
            lines.push(format!("Hint: {hint}"));
        }
        lines
    }
}

/// Structured internal observability log entry (kept distinct from UI error banners).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLogEntry {
    pub timestamp: String,
    pub category: String,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

/// Central DiagnosticEngine storing active per-resource errors, chronological message
/// history (`:messages`), active errors (`:errors`), and internal structured logs.
#[derive(Debug, Clone)]
pub struct DiagnosticEngine {
    active_by_resource: BTreeMap<ResourceId, Vec<Diagnostic>>,
    history: Vec<Diagnostic>,
    logs: Vec<RuntimeLogEntry>,
    max_history: usize,
    max_logs: usize,
}

impl Default for DiagnosticEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticEngine {
    pub fn new() -> Self {
        Self {
            active_by_resource: BTreeMap::new(),
            history: Vec::new(),
            logs: Vec::new(),
            max_history: 256,
            max_logs: 512,
        }
    }

    /// Replaces the active diagnostics for `id` cleanly (empty list clears prior errors for `id`),
    /// while recording every new entry into queryable history.
    pub fn set_resource_diagnostics(&mut self, id: &ResourceId, diags: Vec<Diagnostic>) {
        if diags.is_empty() {
            self.active_by_resource.remove(id);
            return;
        }
        for d in &diags {
            self.push_history(d.clone());
        }
        self.active_by_resource.insert(id.clone(), diags);
    }

    pub fn clear_resource(&mut self, id: &ResourceId) {
        self.active_by_resource.remove(id);
    }

    pub fn record_event(&mut self, diag: Diagnostic) {
        self.push_history(diag);
    }

    pub fn log_config(&mut self, severity: DiagnosticSeverity, message: impl Into<String>) {
        let entry = RuntimeLogEntry {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            category: "Config".to_string(),
            message: message.into(),
            severity,
        };
        self.logs.push(entry);
        if self.logs.len() > self.max_logs {
            let overflow = self.logs.len() - self.max_logs;
            self.logs.drain(0..overflow);
        }
    }

    pub fn active_errors(&self) -> Vec<&Diagnostic> {
        self.active_by_resource
            .values()
            .flatten()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .collect()
    }

    pub fn active_warnings_and_errors(&self) -> Vec<&Diagnostic> {
        self.active_by_resource.values().flatten().collect()
    }

    pub fn error_count(&self) -> usize {
        self.active_errors().len()
    }

    pub fn warning_count(&self) -> usize {
        self.active_by_resource
            .values()
            .flatten()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count()
    }

    pub fn history(&self) -> &[Diagnostic] {
        &self.history
    }

    pub fn logs(&self) -> &[RuntimeLogEntry] {
        &self.logs
    }

    pub fn formatted_log_lines(&self) -> Vec<String> {
        self.logs
            .iter()
            .map(|l| format!("{} [{}] {}", l.timestamp, l.category, l.message))
            .collect()
    }

    fn push_history(&mut self, diag: Diagnostic) {
        self.history.push(diag);
        if self.history.len() > self.max_history {
            let overflow = self.history.len() - self.max_history;
            self.history.drain(0..overflow);
        }
    }
}

/// Converts a UTF-8 byte offset inside `raw` into 1-based `(line, column)`.
pub fn line_col_from_offset(raw: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(raw.len());
    let prefix = &raw[..clamped];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
    let last_nl = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let column = prefix[last_nl..].chars().count() + 1;
    (line, column)
}

/// Attempts to locate the 1-based `(line, column)` of a dotted key path inside TOML text.
pub fn locate_field_in_toml(raw: &str, field_path: &str) -> (Option<usize>, Option<usize>) {
    let leaf = field_path
        .split('.')
        .next_back()
        .unwrap_or(field_path)
        .trim();
    if leaf.is_empty() {
        return (None, None);
    }
    for (idx, line_str) in raw.lines().enumerate() {
        let trimmed = line_str.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(after) = trimmed.strip_prefix(leaf) {
            let rest = after.trim_start();
            if rest.starts_with('=') {
                let col = line_str.chars().count() - trimmed.chars().count() + 1;
                return (Some(idx + 1), Some(col));
            }
        }
    }
    (None, None)
}

/// Finds the approximate `(line, column)` of the value on a matching key assignment line.
pub fn locate_value_in_toml(raw: &str, field_path: &str) -> (Option<usize>, Option<usize>) {
    let leaf = field_path
        .split('.')
        .next_back()
        .unwrap_or(field_path)
        .trim();
    if leaf.is_empty() {
        return (None, None);
    }
    for (idx, line_str) in raw.lines().enumerate() {
        let trimmed = line_str.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(after) = trimmed.strip_prefix(leaf)
            && after.trim_start().starts_with('=')
            && let Some(eq_pos) = line_str.find('=')
        {
            let after_eq = &line_str[eq_pos + 1..];
            let ws = after_eq.len() - after_eq.trim_start().len();
            let col = line_str[..eq_pos + 1 + ws].chars().count() + 1;
            return (Some(idx + 1), Some(col));
        }
    }
    (None, None)
}
