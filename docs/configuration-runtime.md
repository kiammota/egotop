# Unified Live Configuration Runtime

Egotop's live configuration architecture ([`src/runtime/`](../src/runtime/mod.rs)) watches configuration resources on disk, debounces bursts and atomic editor writes, validates updates through a three-stage pipeline, records structured diagnostics, preserves the **Last Known Good** active state on failure, and applies validated changes cleanly to runtime subsystems without restarting or blocking the TUI loop.

---

## Architecture Flow

```text
Filesystem (~/.config/egotop/)
  │
  ▼
FileWatcher (notify::RecommendedWatcher, background thread -> mpsc channel)
  │
  ▼
ChangeDebouncer (120 ms window, collapses rapid & atomic editor events)
  │
  ▼
ConfigurationRuntime (generation-ordered coordinator + ResourceRegistry)
  │
  ├──► Stage 1: parse_resource        (syntax / deserialization -> E1001 / E1003)
  ├──► Stage 2: validate_schema       (unknown keys, types, ranges -> E1002 / E1005)
  ├──► Stage 3: validate_semantics    (colors, missing deps, cycles, keyframes -> E1004 / E1006..E1009)
  │
  ├──[On Failure]──► Discard candidate snapshot, preserve LastKnownGood, publish Diagnostics + Log
  │
  └──[On Success]──► Atomic LastKnownGoodState commit ──► RuntimeResourceManager ──► App State ──► Renderer
```

---

## Subsystems & Contracts

### 1. `ConfigurationRuntime` (`src/runtime/coordinator.rs`)
The central single source of truth coordinating monitored paths, debouncing, validation, dependency propagation, generation ordering, diagnostics, and atomic state commits. It does **not** know how themes render or how animations draw — it delegates application to `RuntimeResourceManager`.

Key methods:
- `ConfigurationRuntime::start(paths, initial_config, initial_window, enable_os_watcher)` — initializes watchers, builds `LastKnownGoodState`, and performs the initial resource scan.
- `poll(&mut self) -> Option<RuntimeCommit>` — non-blocking per-frame poll called by `App::run`. Drains watcher events, debounces, handles error recovery, validates ready changes, and commits only if valid and monotonically newer (`candidate_gen > active_gen`).
- `reload_all(&mut self) -> RuntimeCommit` — shared entry point for `:reload` and `Rescan` recovery (no duplicate parsing logic).
- `process_raw_candidate(...)` — validates an explicit candidate buffer with an explicit generation number, rejecting stale/out-of-order commits.

### 2. `FileWatcher` (`src/runtime/watcher.rs`)
Uses `notify::RecommendedWatcher` on a background thread forwarding normalized `FileWatchEvent { path, kind }` events over a `std::sync::mpsc` channel.
- Deduplicates watched root paths under `~/.config/egotop/` (`egotop.toml`, `window.toml`, `themes/`, `animations/`, `layouts/`, `fonts/`), registering new subdirectories dynamically.
- Normalizes `Created`, `Modified`, `Removed`, `Renamed`, and `Rescan`, ignoring pure read/access events and editor swap/probe artifacts (`*.swp`, `*~`, `.#*`, `4913`).
- Automatically invokes `recover(&roots)` on watcher errors and triggers a safe rescan.

### 3. `ChangeDebouncer` (`src/runtime/debouncer.rs`)
Coalesces bursts from editors such as Neovim (e.g. write-temp -> unlink -> rename atomic save cycles) using a configurable window (default `120 ms`). `Removed` followed quickly by `Created`/`Modified` collapses cleanly into `Modified` so active configurations never flicker during atomic saves.

### 4. `ResourceRegistry` (`src/runtime/registry.rs`)
Tracks every registered resource with a strongly typed `ResourceId { kind: ResourceKind, key }`, `source_path`, monotonic `version`, `LoadStatus::{Unloaded, Loaded, Failed}`, `ValidationStatus::{Pending, Valid, Invalid}`, and `dependencies: Vec<ResourceId>`.

Implemented `ResourceKind` variants:
- `ApplicationConfig` (`egotop.toml`)
- `WindowConfig` (`window.toml`)
- `Theme` (`themes/*.toml`)
- `Animation` (`animations/*.toml` & `animations/<pkg>/animation.toml`)
- `Background` (runtime background pipeline managed via `BackgroundManager`)
- `Layout` (`layouts/*.toml`)
- `Font` (`fonts/*.flf`)

Supports reverse dependency traversal (`dependents_of`) and cycle detection (`detect_cycle`).

### 5. Three-Stage Parsing & Validation Pipeline (`src/runtime/pipeline.rs`)
Each stage emits structured `Diagnostic` values independently:
1. **`parse_resource`**: Parses UTF-8 text via `toml::from_str` / FLF parser and builds typed `ParsedPayload`, mapping `toml::de::Error::span()` byte offsets to exact 1-based `(line, column)`.
2. **`validate_schema`**: Checks allowed fields (catching unknown keys as `E1002`), value ranges, and schema versions (`E1005`).
3. **`validate_semantics`**: Cross-checks color specs (`E1004`), border characters, theme/animation/font references (`E1006`), keyframe bounds (`E1007`), safe relative animation assets (`E1008`), and circular dependencies (`E1009`).

### 6. `DiagnosticEngine` & Structured Logging (`src/runtime/diagnostics.rs`)
Maintains active diagnostics per `ResourceId`, chronological diagnostic history (`:messages`), active warnings/errors (`:errors`), and internal `[Config]` log records (`logs()`). Diagnostics are completely UI-agnostic and carry `severity`, `code`, `file`, `line`, `column`, `field_path`, `expected`, `actual`, and `suggestion`:

```text
ERROR E1004
File: themes/bamboo.toml
Line: 12
Column: 10
Field: theme.border
Invalid color value: "invalid-color"
Expected: #RRGGBB
Actual: "invalid-color"
Hint: Use 6-digit hex like #39FF88, a standard color name, or 'reset'
```

### 7. `LastKnownGoodState` (`src/runtime/state.rs`)
Holds the latest committed generation and complete validated snapshots (`Config`, `WindowConfig`, themes, animations, layouts, fonts). Any parse, schema, semantic, or generation ordering failure discards the candidate batch atomically so active visuals are never half-applied or corrupted.

### 8. `RuntimeResourceManager` (`src/runtime/managers.rs`)
Applies only the dirty subsystems reported in `RuntimeCommit` via focused managers (`AppConfigManager`, `WindowConfigManager`, `ThemeManager`, `AnimationManager`, `BackgroundManager`, `LayoutManager`, `FontManager`) into `ManagedAppTarget`.

---

## TUI Integration & Commands
- **Header Indicator**: When `config_error_count > 0`, the top status bar persistently shows `[N err • :errors]` in `theme.danger` even after transient toasts fade.
- **Interactive Overlay (`DiagnosticsOverlayState`)**: Rendered via `render_diagnostics_overlay` (`src/ui/customizer.rs`) honoring active theme colors, border characters, z-order, scrollback, and `Tab` switching between active **Errors** and chronological **Messages & Logs**.
- **Commands**:
  - `:reload` (`:r`) — runs `ConfigurationRuntime::reload_all` and applies valid changes atomically.
  - `:errors` (`:err`) — opens active configuration errors with file, line, column, expected/actual values, and hints.
  - `:messages` (`:msg`) — opens structured `[Config]` logs and chronological diagnostic history.

---

## Extending the Runtime Architecture

### Adding a New Resource Type
1. Add the variant to `ResourceKind` in [`src/runtime/registry.rs`](../src/runtime/registry.rs).
2. Add its file path classification in `ConfigurationRuntime::classify_path` (`src/runtime/coordinator.rs`) and add a `ParsedPayload` variant in `src/runtime/pipeline.rs`.
3. Register its field/semantic rules in `validate_schema` / `validate_semantics` and wire candidate staging in `apply_parsed_to_candidate`.

### Integrating a New Resource Manager
1. Add the updated flag to `RuntimeCommit` (`src/runtime/state.rs`) and field reference to `ManagedAppTarget` (`src/runtime/managers.rs`).
2. Create a dedicated manager struct (e.g. `MyResourceManager::apply(lkg, target)`) and invoke it conditionally inside `RuntimeResourceManager::apply_commit`.

### Registering New Validators
1. Add structural checks inside `validate_schema` (`check_unknown_fields` / range checks) for new TOML keys or limits, returning `E1002`/`E1005`.
2. Add cross-resource or domain checks inside `validate_semantics` using `ValidationContext` (`themes`, `animations`, `fonts`, `registry`), emitting structured `Diagnostic` entries with exact `with_location`, `with_field`, and `with_expected_actual`.
