# Architecture

Egotop separates configuration, system collection, automatic layout, isolated canvas rendering, background effects, animations, command processing, and terminal I/O into independent modules under `src/`.

## Core Rendering & Layout Model

1. **Flat Independent Containers (`src/ui/container.rs`)**:
   - Containers are self-contained, non-nestable units (`ContainerConfig`) identified by `name` and `description`.
   - A container **cannot contain another container** (there is no recursive node tree).
   - Each container owns an exclusive, strictly bounded rendering canvas and holds any supported content (`text`, `greeting`, `header`, `system`, `hardware`, `activity`, `graph`, `ascii_logo`, `matrix`, `particles`, `ascii_object`, `frames`, `typewriter`, `spinner`, or Lua `canvas` / inline `render(canvas, ctx)`).

2. **Automatic Queue & Region Layout (`src/ui/layout.rs`)**:
   - Users declare a flat queue of containers in `egotop.layout({ containers = { ... } })` (with optional `region` = `top` | `left` | `main` | `right` | `bottom`, `row`, `width`, `height`, `min_*`/`max_*`).
   - Egotop distributes containers left-to-right, calculates balanced proportional dimensions, wraps rows automatically when terminal width changes, prevents overlap, and recomputes geometry on every terminal resize.

3. **Independent Background Layer (`src/terminal/renderer.rs`, `src/animation/backgrounds.rs`)**:
   - The background is **not** a container and does not participate in container layout distribution.
   - It owns an independent background surface (`name = "background"`) and composites only into areas outside excluded container bounds (`ExclusionMask`).

4. **Strict Canvas Isolation (`src/lua/canvas.rs`, `src/ui/components.rs`, `src/ui/widgets.rs`)**:
   - Every container, the background layer, and every independent animation (`LuaAnimationRuntime`) renders into its own isolated local `[0..width, 0..height)` buffer/canvas.
   - Scripts and widgets never receive global terminal coordinates or references to other canvases. All draws are clipped locally before compositing onto the frame.

## Module Responsibilities

| Module | Path | Responsibility |
|---|---|---|
| **CLI Engine** | [`src/cli/`](../src/cli/mod.rs) | Modular `clap` administrative subcommands (`config`, `fonts`, `context`, `doctor`), global options, and TUI startup dispatch. |
| **Application Coordinator** | [`src/app.rs`](../src/app.rs) | Holds runtime `App` state, decouples animation tick rate (`animation_fps`) from system refresh rate (`refresh_rate_ms`), routes events, and runs commands. |
| **Configuration Runtime** | [`src/runtime/`](../src/runtime/mod.rs) | Centralized live-reload architecture (`ConfigurationRuntime`, `notify`-backed `FileWatcher`, `ChangeDebouncer`, `ResourceRegistry`, 3-stage `pipeline`, `DiagnosticEngine`, `LastKnownGoodState`, `RuntimeResourceManager`). |
| **Configuration Engine** | [`src/config/`](../src/config/mod.rs) | Path resolution (`paths.rs`), validated config schema (`schema.rs`), persistence (`loader.rs`), runtime theme scanner (`theme_loader.rs`), and animation package loader (`animation_loader.rs`). |
| **Lua Engine & Isolated Canvas** | [`src/lua/`](../src/lua/mod.rs) | Lua 5.4 configuration API (`api.rs`, `serializer.rs`), isolated `CanvasBuffer` / `LuaCanvasHandle` (`canvas.rs`), per-unit isolated shader/effect VMs (`effects.rs`), and isolated animation runtime (`animation_runtime.rs`). |
| **Command Engine** | [`src/command/`](../src/command/mod.rs) | `CommandRegistry` (`registry.rs`), `CompletionEngine` (`completion.rs`), `parse_command` (`parser.rs`), and `CommandLineState` (`mod.rs`). |
| **Animation Engine** | [`src/animation/`](../src/animation/mod.rs) | Unified declarative Scene/Timeline engine and composable multi-layer `BackgroundPipeline`. |
| **Rendering & UI** | [`src/ui/`](../src/ui/mod.rs) | Flat `ContainerConfig` (`container.rs`), automatic linear/region layout solver (`layout.rs`), isolated canvas compositing (`components.rs`, `widgets.rs`), `BorderChars` (`borders.rs`), `Theme` (`theme.rs`), FIGlet fonts (`fonts.rs`, `typography.rs`), and overlays (`customizer.rs`). |
| **Terminal & Input** | [`src/terminal/`](../src/terminal/mod.rs) | RAII `TerminalGuard` + panic hook (`mod.rs`), event polling (`input.rs`), and top-level frame composition (`renderer.rs`). |
| **System Collector** | [`src/system/`](../src/system/mod.rs) | Rootless Linux metrics collection via `/etc/os-release`, `libc::uname`, `libc::statvfs`, `procfs`, `/proc`, `/sys/class/hwmon`, and `/sys/class/drm`. |
