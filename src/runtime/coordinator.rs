use crate::animation::{AnimationCatalog, AnimationCatalogEntry};
use crate::config::{Config, ThemeEntry, paths};
use crate::runtime::debouncer::{ChangeDebouncer, PendingChange, WatchEventKind};
use crate::runtime::diagnostics::{Diagnostic, DiagnosticEngine, DiagnosticSeverity};
use crate::runtime::pipeline::{
    LayoutPreset, ParsedPayload, ParsedResource, ValidationContext, builtin_themes_lookup,
    parse_resource, validate_schema, validate_semantics,
};
use crate::runtime::registry::{LoadStatus, ResourceId, ResourceKind, ResourceRegistry, ValidationStatus};
use crate::runtime::state::{LastKnownGoodState, RuntimeCommit};
use crate::runtime::watcher::FileWatcher;
use crate::window::WindowConfig;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub root_dir: PathBuf,
    pub config_path: PathBuf,
    pub window_path: PathBuf,
    pub themes_dir: PathBuf,
    pub animations_dir: PathBuf,
    pub layouts_dir: PathBuf,
    pub fonts_dir: PathBuf,
}

impl RuntimePaths {
    pub fn default_with_config(config_path: PathBuf) -> Self {
        Self {
            root_dir: paths::root_path(),
            config_path,
            window_path: paths::window_config_path(),
            themes_dir: paths::themes_path(),
            animations_dir: paths::animations_path(),
            layouts_dir: paths::layouts_path(),
            fonts_dir: paths::fonts_path(),
        }
    }

    pub fn from_root_dir(root: PathBuf) -> Self {
        Self {
            config_path: root.join("egotop.toml"),
            window_path: root.join("window.toml"),
            themes_dir: root.join("themes"),
            animations_dir: root.join("animations"),
            layouts_dir: root.join("layouts"),
            fonts_dir: root.join("fonts"),
            root_dir: root,
        }
    }

    pub fn watch_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.root_dir.clone()];
        if let Some(parent) = self.config_path.parent()
            && !parent.as_os_str().is_empty()
            && !roots.iter().any(|r| parent.starts_with(r))
        {
            roots.push(parent.to_path_buf());
        }
        for dir in [
            &self.themes_dir,
            &self.animations_dir,
            &self.layouts_dir,
            &self.fonts_dir,
        ] {
            if !roots.iter().any(|r| dir.starts_with(r)) {
                roots.push(dir.clone());
            }
        }
        roots
    }
}

/// Central coordinator managing watched files, debouncing, validation pipeline,
/// ResourceRegistry, LastKnownGood state, generation ordering, and Diagnostics.
pub struct ConfigurationRuntime {
    paths: RuntimePaths,
    watcher: FileWatcher,
    debouncer: ChangeDebouncer,
    registry: ResourceRegistry,
    diagnostics: DiagnosticEngine,
    lkg: LastKnownGoodState,
    next_generation: u64,
}

impl ConfigurationRuntime {
    pub fn start(
        paths: RuntimePaths,
        initial_config: Config,
        initial_window: WindowConfig,
        enable_os_watcher: bool,
    ) -> Self {
        let roots = paths.watch_roots();
        let watcher = if enable_os_watcher {
            FileWatcher::start(&roots)
        } else {
            FileWatcher::offline()
        };

        let mut runtime = Self {
            paths,
            watcher,
            debouncer: ChangeDebouncer::new(Duration::from_millis(120)),
            registry: ResourceRegistry::new(),
            diagnostics: DiagnosticEngine::new(),
            lkg: LastKnownGoodState::new(
                initial_config,
                initial_window,
                builtin_themes_lookup(),
                AnimationCatalog::load(),
                BTreeMap::new(),
                BTreeSet::new(),
            ),
            next_generation: 2,
        };

        runtime.diagnostics.log_config(
            DiagnosticSeverity::Info,
            "ConfigurationRuntime initialized",
        );
        let _ = runtime.reload_all();
        runtime
    }

    pub fn paths(&self) -> &RuntimePaths {
        &self.paths
    }

    pub fn state(&self) -> &LastKnownGoodState {
        &self.lkg
    }

    pub fn registry(&self) -> &ResourceRegistry {
        &self.registry
    }

    pub fn diagnostics(&self) -> &DiagnosticEngine {
        &self.diagnostics
    }

    pub fn diagnostics_mut(&mut self) -> &mut DiagnosticEngine {
        &mut self.diagnostics
    }

    pub fn watcher(&self) -> &FileWatcher {
        &self.watcher
    }

    pub fn watcher_mut(&mut self) -> &mut FileWatcher {
        &mut self.watcher
    }

    pub fn debouncer_mut(&mut self) -> &mut ChangeDebouncer {
        &mut self.debouncer
    }

    /// Polls non-blockingly for debounced filesystem events and applies validated updates.
    pub fn poll(&mut self) -> Option<RuntimeCommit> {
        let (events, watch_errors) = self.watcher.drain_events();
        if !watch_errors.is_empty() {
            for err in watch_errors {
                self.diagnostics.log_config(
                    DiagnosticSeverity::Warning,
                    format!("FileWatcher error: {err} — recovering watch handles"),
                );
            }
            let roots = self.paths.watch_roots();
            self.watcher.recover(&roots);
        }

        let now = Instant::now();
        for ev in events {
            if self.classify_path(&ev.path).is_some() || ev.kind == WatchEventKind::Rescan {
                self.debouncer.push(ev.path, ev.kind, now);
            }
        }

        let (ready, need_rescan) = self.debouncer.flush_ready(now);
        if need_rescan {
            self.diagnostics.log_config(
                DiagnosticSeverity::Info,
                "Rescanning all configuration resources after watcher recovery",
            );
            return Some(self.reload_all());
        }

        if ready.is_empty() {
            return None;
        }

        let roots = self.paths.watch_roots();
        self.watcher.ensure_roots_watched(&roots);
        Some(self.process_changes(&ready))
    }

    /// Manually reloads all watched configuration files through the unified pipeline.
    pub fn reload_all(&mut self) -> RuntimeCommit {
        self.diagnostics.log_config(
            DiagnosticSeverity::Info,
            "Manual/full reload requested",
        );

        let mut batch = Vec::new();
        if self.paths.window_path.is_file() {
            batch.push(PendingChange {
                path: self.paths.window_path.clone(),
                kind: WatchEventKind::Modified,
                last_event_at: Instant::now(),
                event_count: 1,
            });
        }
        collect_toml_entries(&self.paths.themes_dir, &mut batch);
        collect_animation_entries(&self.paths.animations_dir, &mut batch);
        collect_toml_entries(&self.paths.layouts_dir, &mut batch);
        collect_font_entries(&self.paths.fonts_dir, &mut batch);
        if self.paths.config_path.is_file() {
            batch.push(PendingChange {
                path: self.paths.config_path.clone(),
                kind: WatchEventKind::Modified,
                last_event_at: Instant::now(),
                event_count: 1,
            });
        }

        // Also detect files that disappeared from disk since the previous snapshot
        let missing: Vec<PathBuf> = self
            .registry
            .all()
            .filter(|rec| !rec.source_path.exists())
            .map(|rec| rec.source_path.clone())
            .collect();
        for path in missing {
            batch.push(PendingChange {
                path,
                kind: WatchEventKind::Removed,
                last_event_at: Instant::now(),
                event_count: 1,
            });
        }

        if batch.is_empty() {
            return RuntimeCommit {
                generation: self.lkg.generation,
                summary: "Configuration up to date (no custom files found)".to_string(),
                ..RuntimeCommit::default()
            };
        }

        self.process_changes(&batch)
    }

    /// Evaluates an explicit candidate in memory against a target `generation` ID
    /// (discarding stale out-of-order updates deterministically).
    pub fn process_raw_candidate(
        &mut self,
        kind: ResourceKind,
        key_hint: &str,
        path: &Path,
        raw_text: String,
        candidate_generation: u64,
    ) -> RuntimeCommit {
        if candidate_generation <= self.lkg.generation {
            self.diagnostics.log_config(
                DiagnosticSeverity::Warning,
                format!(
                    "Discarded out-of-order candidate generation {} (active: {}) for {}",
                    candidate_generation,
                    self.lkg.generation,
                    path.display()
                ),
            );
            return RuntimeCommit {
                generation: self.lkg.generation,
                had_errors: true,
                summary: "Discarded stale out-of-order update".to_string(),
                ..RuntimeCommit::default()
            };
        }

        let mut next_themes = self.lkg.themes.clone();
        let mut next_animations = self.lkg.animations.clone();
        let mut next_layouts = self.lkg.layouts.clone();
        let mut next_fonts = self.lkg.custom_fonts.clone();
        let mut next_config = self.lkg.config.clone();
        let mut next_window = self.lkg.window.clone();
        let mut commit = RuntimeCommit {
            generation: candidate_generation,
            ..RuntimeCommit::default()
        };

        let ok = self.run_pipeline_on_text(
            kind,
            key_hint,
            path,
            raw_text,
            &mut next_config,
            &mut next_window,
            &mut next_themes,
            &mut next_animations,
            &mut next_layouts,
            &mut next_fonts,
            &mut commit,
        );

        if !ok || !self.lkg.try_commit_generation(candidate_generation) {
            commit.had_errors = true;
            commit.generation = self.lkg.generation;
            self.diagnostics.log_config(
                DiagnosticSeverity::Error,
                "Validation failed. Previous configuration preserved.",
            );
            return commit;
        }

        self.lkg.config = next_config;
        self.lkg.window = next_window;
        self.lkg.themes = next_themes;
        self.lkg.animations = next_animations;
        self.lkg.layouts = next_layouts;
        self.lkg.custom_fonts = next_fonts;
        commit.summary = format!("Committed generation {}", self.lkg.generation);
        commit
    }

    fn alloc_generation(&mut self) -> u64 {
        let g = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        g
    }

    fn process_changes(&mut self, changes: &[PendingChange]) -> RuntimeCommit {
        let generation = self.alloc_generation();
        let mut commit = RuntimeCommit {
            generation,
            ..RuntimeCommit::default()
        };

        let mut next_config = self.lkg.config.clone();
        let mut next_window = self.lkg.window.clone();
        let mut next_themes = self.lkg.themes.clone();
        let mut next_animations = self.lkg.animations.clone();
        let mut next_layouts = self.lkg.layouts.clone();
        let mut next_fonts = self.lkg.custom_fonts.clone();

        // Normalize target paths (so e.g. pkg/assets/core.txt resolves to pkg/animation.toml)
        let mut unique_targets: Vec<(ResourceKind, String, PathBuf, WatchEventKind)> = Vec::new();
        for ch in changes {
            let Some((kind, key, effective_path)) = self.classify_path(&ch.path) else {
                continue;
            };
            if let Some(existing) = unique_targets.iter_mut().find(|(_, _, p, _)| *p == effective_path) {
                existing.3 = ch.kind;
            } else {
                unique_targets.push((kind, key, effective_path, ch.kind));
            }
        }

        // Order dependency roots before dependents: Fonts/Themes/Animations/Layouts -> Window -> Config
        unique_targets.sort_by_key(|(k, _, _, _)| resource_sort_order(*k));

        let mut all_ok = true;
        let mut touched_ids = Vec::new();

        for (kind, key, path, ev_kind) in unique_targets {
            self.diagnostics.log_config(
                DiagnosticSeverity::Info,
                format!("Change detected: {} ({ev_kind:?})", path.display()),
            );

            if ev_kind == WatchEventKind::Removed || !path.exists() {
                let removed = self.handle_removal(
                    kind,
                    &key,
                    &path,
                    &mut next_themes,
                    &mut next_animations,
                    &mut next_layouts,
                    &mut next_fonts,
                    &mut commit,
                );
                if !removed {
                    all_ok = false;
                } else {
                    touched_ids.push(ResourceId::new(kind, &key));
                }
                continue;
            }

            let Ok(raw_text) = fs::read_to_string(&path) else {
                let id = ResourceId::new(kind, &key);
                let diag = Diagnostic::error("E1000", "Failed to read configuration file")
                    .with_location(Some(&path), None, None)
                    .with_suggestion("Verify file permissions and valid UTF-8 encoding");
                self.diagnostics.set_resource_diagnostics(&id, vec![diag]);
                self.registry
                    .upsert(id, path.clone(), LoadStatus::Failed, ValidationStatus::Invalid, Vec::new());
                all_ok = false;
                continue;
            };

            let step_ok = self.run_pipeline_on_text(
                kind,
                &key,
                &path,
                raw_text,
                &mut next_config,
                &mut next_window,
                &mut next_themes,
                &mut next_animations,
                &mut next_layouts,
                &mut next_fonts,
                &mut commit,
            );
            if !step_ok {
                all_ok = false;
            } else {
                touched_ids.push(ResourceId::new(kind, &key));
            }
        }

        // Revalidate dependents (e.g. egotop.toml if its referenced theme/animation/font was removed or changed)
        let mut dependent_paths = Vec::new();
        for tid in &touched_ids {
            for dep_id in self.registry.dependents_of(tid) {
                if !touched_ids.contains(&dep_id)
                    && let Some(rec) = self.registry.get(&dep_id)
                {
                    dependent_paths.push((dep_id.kind, dep_id.key.clone(), rec.source_path.clone()));
                }
            }
        }
        for (dkind, dkey, dpath) in dependent_paths {
            if dpath.is_file()
                && let Ok(raw) = fs::read_to_string(&dpath)
                && !self.run_pipeline_on_text(
                    dkind,
                    &dkey,
                    &dpath,
                    raw,
                    &mut next_config,
                    &mut next_window,
                    &mut next_themes,
                    &mut next_animations,
                    &mut next_layouts,
                    &mut next_fonts,
                    &mut commit,
                )
            {
                all_ok = false;
            }
        }

        if !all_ok {
            commit.had_errors = true;
            commit.generation = self.lkg.generation;
            commit.summary = format!(
                "Reload failed ({} error(s)) — previous configuration preserved",
                self.diagnostics.error_count()
            );
            self.diagnostics.log_config(
                DiagnosticSeverity::Error,
                "Validation failed. Previous configuration preserved.",
            );
            return commit;
        }

        if self.lkg.try_commit_generation(generation) {
            self.lkg.config = next_config;
            self.lkg.window = next_window;
            self.lkg.themes = next_themes;
            self.lkg.animations = next_animations;
            self.lkg.layouts = next_layouts;
            self.lkg.custom_fonts = next_fonts;
            commit.summary = "Configuration successfully reloaded".to_string();
            self.diagnostics.log_config(
                DiagnosticSeverity::Info,
                "Resource successfully reloaded.",
            );
        }

        commit
    }

    #[allow(clippy::too_many_arguments)]
    fn run_pipeline_on_text(
        &mut self,
        kind: ResourceKind,
        key_hint: &str,
        path: &Path,
        raw_text: String,
        next_config: &mut Config,
        next_window: &mut WindowConfig,
        next_themes: &mut BTreeMap<String, ThemeEntry>,
        next_animations: &mut AnimationCatalog,
        next_layouts: &mut BTreeMap<String, LayoutPreset>,
        next_fonts: &mut BTreeSet<String>,
        commit: &mut RuntimeCommit,
    ) -> bool {
        let fallback_id = ResourceId::new(kind, key_hint);
        self.diagnostics
            .log_config(DiagnosticSeverity::Info, "Parsing resource...");

        let parsed = match parse_resource(kind, key_hint, path, raw_text) {
            Ok(p) => p,
            Err(diags) => {
                self.diagnostics.set_resource_diagnostics(&fallback_id, diags);
                self.registry.upsert(
                    fallback_id,
                    path.to_path_buf(),
                    LoadStatus::Failed,
                    ValidationStatus::Invalid,
                    Vec::new(),
                );
                return false;
            }
        };

        let mut schema_diags = validate_schema(&parsed);
        if schema_diags
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
        {
            self.diagnostics
                .set_resource_diagnostics(&parsed.id, schema_diags);
            self.registry.upsert(
                parsed.id.clone(),
                path.to_path_buf(),
                LoadStatus::Failed,
                ValidationStatus::Invalid,
                parsed.dependencies.clone(),
            );
            return false;
        }
        self.diagnostics
            .log_config(DiagnosticSeverity::Info, "Schema validation passed.");

        let sem_diags = {
            let vctx = ValidationContext {
                themes: next_themes,
                animations: next_animations,
                fonts: next_fonts,
                registry: &self.registry,
            };
            validate_semantics(&parsed, &vctx)
        };
        schema_diags.extend(sem_diags);

        if schema_diags
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
        {
            self.diagnostics
                .set_resource_diagnostics(&parsed.id, schema_diags);
            self.registry.upsert(
                parsed.id.clone(),
                path.to_path_buf(),
                LoadStatus::Failed,
                ValidationStatus::Invalid,
                parsed.dependencies.clone(),
            );
            return false;
        }
        self.diagnostics
            .log_config(DiagnosticSeverity::Info, "Semantic validation passed.");

        // Commit warnings (if any) without blocking resource activation
        self.diagnostics
            .set_resource_diagnostics(&parsed.id, schema_diags);
        self.registry.upsert(
            parsed.id.clone(),
            path.to_path_buf(),
            LoadStatus::Loaded,
            ValidationStatus::Valid,
            parsed.dependencies.clone(),
        );

        apply_parsed_to_candidate(
            parsed,
            next_config,
            next_window,
            next_themes,
            next_animations,
            next_layouts,
            next_fonts,
            commit,
        );
        self.diagnostics
            .log_config(DiagnosticSeverity::Info, "Applying resource...");
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_removal(
        &mut self,
        kind: ResourceKind,
        key: &str,
        path: &Path,
        next_themes: &mut BTreeMap<String, ThemeEntry>,
        next_animations: &mut AnimationCatalog,
        next_layouts: &mut BTreeMap<String, LayoutPreset>,
        next_fonts: &mut BTreeSet<String>,
        commit: &mut RuntimeCommit,
    ) -> bool {
        if kind == ResourceKind::ApplicationConfig || kind == ResourceKind::WindowConfig {
            let id = ResourceId::new(kind, key);
            let diag = Diagnostic::warning(
                "W2002",
                format!(
                    "Configuration file {} was removed — LastKnownGood state preserved",
                    path.display()
                ),
            )
            .with_location(Some(path), None, None);
            self.diagnostics.record_event(diag);
            self.registry
                .update_status(&id, LoadStatus::Failed, ValidationStatus::Invalid);
            return true;
        }

        let removed_ids = self.registry.remove_by_path(path);
        for id in &removed_ids {
            self.diagnostics.clear_resource(id);
        }

        match kind {
            ResourceKind::Theme => {
                next_themes.retain(|_, entry| entry.source_path.as_deref() != Some(path));
                // Restore built-in fallback if removed file overrode a built-in name
                let builtins = builtin_themes_lookup();
                for (bkey, bval) in builtins {
                    next_themes.entry(bkey).or_insert(bval);
                }
                commit.themes_updated = true;
            }
            ResourceKind::Animation => {
                // Rebuild catalog cleanly keeping bundled entries + remaining valid disk packages
                let mut rebuilt = AnimationCatalog::load();
                rebuilt.entries.retain(|e| !e.id.eq_ignore_ascii_case(key) || !e.is_community);
                *next_animations = rebuilt;
                commit.animations_updated = true;
            }
            ResourceKind::Layout => {
                next_layouts.remove(&key.to_ascii_lowercase());
                commit.layouts_updated = true;
            }
            ResourceKind::Font => {
                next_fonts.remove(&key.to_ascii_lowercase());
                commit.fonts_updated = true;
            }
            _ => {}
        }

        self.diagnostics.record_event(
            Diagnostic::info(
                "I0001",
                format!("Resource removed from disk: {}", path.display()),
            )
            .with_location(Some(path), None, None),
        );
        true
    }

    fn classify_path(&self, raw_path: &Path) -> Option<(ResourceKind, String, PathBuf)> {
        let p = fs::canonicalize(raw_path).unwrap_or_else(|_| raw_path.to_path_buf());
        let cfg_norm =
            fs::canonicalize(&self.paths.config_path).unwrap_or_else(|_| self.paths.config_path.clone());
        let win_norm =
            fs::canonicalize(&self.paths.window_path).unwrap_or_else(|_| self.paths.window_path.clone());

        if p == cfg_norm || raw_path == self.paths.config_path {
            return Some((
                ResourceKind::ApplicationConfig,
                "egotop".to_string(),
                self.paths.config_path.clone(),
            ));
        }
        if p == win_norm || raw_path == self.paths.window_path {
            return Some((
                ResourceKind::WindowConfig,
                "window".to_string(),
                self.paths.window_path.clone(),
            ));
        }

        let themes_norm =
            fs::canonicalize(&self.paths.themes_dir).unwrap_or_else(|_| self.paths.themes_dir.clone());
        if (p.starts_with(&themes_norm) || raw_path.starts_with(&self.paths.themes_dir))
            && p.extension().and_then(|e| e.to_str()) == Some("toml")
        {
            let key = p.file_stem().and_then(|s| s.to_str()).unwrap_or("custom").to_string();
            return Some((ResourceKind::Theme, key, raw_path.to_path_buf()));
        }

        let anim_norm = fs::canonicalize(&self.paths.animations_dir)
            .unwrap_or_else(|_| self.paths.animations_dir.clone());
        if p.starts_with(&anim_norm) || raw_path.starts_with(&self.paths.animations_dir) {
            // Asset or manifest inside <animations>/<pkg>/... maps to <pkg>/animation.toml
            let rel = p
                .strip_prefix(&anim_norm)
                .or_else(|_| raw_path.strip_prefix(&self.paths.animations_dir))
                .ok()?;
            let mut comps = rel.components();
            let first = comps.next()?.as_os_str().to_str()?.to_string();
            let pkg_dir = self.paths.animations_dir.join(&first);
            let pkg_manifest = pkg_dir.join("animation.toml");
            if pkg_dir.is_dir() && pkg_manifest.is_file() {
                return Some((ResourceKind::Animation, first, pkg_manifest));
            }
            if p.extension().and_then(|e| e.to_str()) == Some("toml") {
                let key = first.trim_end_matches(".toml").to_string();
                return Some((ResourceKind::Animation, key, raw_path.to_path_buf()));
            }
        }

        let layouts_norm =
            fs::canonicalize(&self.paths.layouts_dir).unwrap_or_else(|_| self.paths.layouts_dir.clone());
        if (p.starts_with(&layouts_norm) || raw_path.starts_with(&self.paths.layouts_dir))
            && p.extension().and_then(|e| e.to_str()) == Some("toml")
        {
            let key = p.file_stem().and_then(|s| s.to_str()).unwrap_or("custom").to_string();
            return Some((ResourceKind::Layout, key, raw_path.to_path_buf()));
        }

        let fonts_norm =
            fs::canonicalize(&self.paths.fonts_dir).unwrap_or_else(|_| self.paths.fonts_dir.clone());
        if (p.starts_with(&fonts_norm) || raw_path.starts_with(&self.paths.fonts_dir))
            && p.extension().and_then(|e| e.to_str()) == Some("flf")
        {
            let key = p.file_stem().and_then(|s| s.to_str()).unwrap_or("custom").to_string();
            return Some((ResourceKind::Font, key, raw_path.to_path_buf()));
        }

        None
    }
}

fn resource_sort_order(kind: ResourceKind) -> u8 {
    match kind {
        ResourceKind::Font => 0,
        ResourceKind::Theme => 1,
        ResourceKind::Animation => 2,
        ResourceKind::Background => 3,
        ResourceKind::Layout => 4,
        ResourceKind::WindowConfig => 5,
        ResourceKind::ApplicationConfig => 6,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_parsed_to_candidate(
    parsed: ParsedResource,
    next_config: &mut Config,
    next_window: &mut WindowConfig,
    next_themes: &mut BTreeMap<String, ThemeEntry>,
    next_animations: &mut AnimationCatalog,
    next_layouts: &mut BTreeMap<String, LayoutPreset>,
    next_fonts: &mut BTreeSet<String>,
    commit: &mut RuntimeCommit,
) {
    match parsed.payload {
        ParsedPayload::ApplicationConfig(cfg) => {
            *next_config = *cfg;
            commit.config_updated = true;
        }
        ParsedPayload::WindowConfig(win) => {
            *next_window = *win;
            commit.window_updated = true;
        }
        ParsedPayload::Theme(entry) => {
            next_themes.insert(entry.name.to_ascii_lowercase(), *entry);
            commit.themes_updated = true;
        }
        ParsedPayload::Animation(pkg) => {
            let def = &pkg.definition;
            let entry = AnimationCatalogEntry {
                id: def.name.clone(),
                display_name: def.display_name.clone().unwrap_or_else(|| def.name.clone()),
                description: def.description.clone().unwrap_or_else(|| {
                    format!(
                        "Declarative animation ({} tracks, {:.1}s)",
                        def.tracks.len(),
                        def.duration_ms as f32 / 1000.0
                    )
                }),
                is_community: true,
                package: *pkg,
            };
            if let Some(slot) = next_animations
                .entries
                .iter_mut()
                .find(|e| e.id.eq_ignore_ascii_case(&entry.id))
            {
                *slot = entry;
            } else {
                next_animations.entries.push(entry);
            }
            commit.animations_updated = true;
        }
        ParsedPayload::Layout(preset) => {
            next_layouts.insert(preset.name.to_ascii_lowercase(), *preset);
            commit.layouts_updated = true;
        }
        ParsedPayload::Font { name, .. } => {
            next_fonts.insert(name.to_ascii_lowercase());
            commit.fonts_updated = true;
        }
    }
}

fn collect_toml_entries(dir: &Path, out: &mut Vec<PendingChange>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            out.push(PendingChange {
                path,
                kind: WatchEventKind::Modified,
                last_event_at: Instant::now(),
                event_count: 1,
            });
        }
    }
}

fn collect_animation_entries(dir: &Path, out: &mut Vec<PendingChange>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest = path.join("animation.toml");
            if manifest.is_file() {
                out.push(PendingChange {
                    path: manifest,
                    kind: WatchEventKind::Modified,
                    last_event_at: Instant::now(),
                    event_count: 1,
                });
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            out.push(PendingChange {
                path,
                kind: WatchEventKind::Modified,
                last_event_at: Instant::now(),
                event_count: 1,
            });
        }
    }
}

fn collect_font_entries(dir: &Path, out: &mut Vec<PendingChange>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("flf") {
            out.push(PendingChange {
                path,
                kind: WatchEventKind::Modified,
                last_event_at: Instant::now(),
                event_count: 1,
            });
        }
    }
}
