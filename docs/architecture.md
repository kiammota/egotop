# Architecture

Egotop separates configuration, system collection, window management, rendering, animations, command processing, and terminal I/O into independent modules under `src/`.

## Module Responsibilities

| Module | Path | Responsibility |
|---|---|---|
| **Application Coordinator** | [`src/app.rs`](../src/app.rs) | Holds runtime `App` state, decouples animation tick rate (`animation_fps`) from system refresh rate (`refresh_rate_ms`), routes events, and runs commands without performing disk reads per frame. |
| **Window Engine** | [`src/window/mod.rs`](../src/window/mod.rs) | `WindowManager` and `WindowState` handle independent window positioning (`Relative`/`Absolute`), anchors, clamping on terminal resize, z-index ordering, mouse drag/resize hit-testing, borders, padding, and solid vs. terminal-cell transparent backgrounds. |
| **Configuration Engine** | [`src/config/`](../src/config/mod.rs) | Path resolution (`paths.rs`), validated TOML schema (`schema.rs`), config/window persistence (`loader.rs`), runtime theme scanner (`theme_loader.rs`), and community animation scanner (`animation_loader.rs`). |
| **Command Engine** | [`src/command/`](../src/command/mod.rs) | `CommandRegistry` (`registry.rs`), `CompletionEngine` (`completion.rs`), `parse_command` (`parser.rs`), and `CommandLineState` (`mod.rs`) for shell-style Tab/Shift+Tab/Space completion, cursor editing, and history. |
| **Animation Engine** | [`src/animation/`](../src/animation/mod.rs) | `BackgroundEffect` trait + `create_effect` factory (`matrix.rs`, `water.rs`, `particles.rs`, `scanlines.rs`, `none.rs`) and `TransitionPlayer` (`transition.rs`) for native transitions and declarative TOML sequences. |
| **Rendering & UI** | [`src/ui/`](../src/ui/mod.rs) | Centralized `BorderChars` (`borders.rs`), `Theme` (`theme.rs`), FIGlet `.flf` parser + built-in fonts (`fonts.rs`, `typography.rs`), responsive/free/compact layout solver (`layout.rs`), widgets/command-line/popup renderer (`widgets.rs`), and interactive Customizer/Help overlays (`customizer.rs`). |
| **Terminal & Input** | [`src/terminal/`](../src/terminal/mod.rs) | RAII `TerminalGuard` + panic hook (`mod.rs`) ensuring raw mode, mouse capture, alternate screen, cursor, and colors are always restored; non-blocking event polling (`input.rs`); top-level frame composition & cursor startup view (`renderer.rs`). |
| **System Collector** | [`src/system/`](../src/system/mod.rs) | Rootless Linux metrics collection via `/etc/os-release`, `libc::uname`, `libc::statvfs`, `procfs`, `/proc`, `/sys/class/hwmon`, and `/sys/class/drm`. |

## Extending Egotop

### Adding a New Command

1. Add a `CommandDescriptor` entry to `CommandRegistry::new()` in `src/command/registry.rs` with `name`, `aliases`, `description`, `usage`, `args`, and `provider`.
2. Add the corresponding variant to `ParsedCommand` and `parse_command()` in `src/command/parser.rs`.
3. If the command accepts dynamic arguments, add its `CompletionProviderKind` branch in `CompletionEngine::complete()` (`src/command/completion.rs`).
4. Handle the `ParsedCommand` variant inside `App::execute_command_str()` in `src/app.rs`.

### Adding a New Theme

No Rust code changes are required: place a `.toml` file inside `~/.config/egotop/themes/` (see [themes.md](themes.md)) and run `:reload` or restart Egotop. To add a compiled-in preset, add it to `builtin_theme_names()` and `preset_theme()` in `src/ui/theme.rs`.

### Adding a New Component / Widget

1. Add a variant to `WidgetId` in `src/config/schema.rs` and define its `default_title()`.
2. Add its rendering function in `src/ui/widgets.rs` and dispatch it inside `render_dashboard()`.
3. Enable or place it via `[[widgets]]` in `~/.config/egotop/egotop.toml`.

### Adding a New Background Effect

1. Implement `BackgroundEffect` (`kind`, `resize`, `update`, `render`) in a new module under `src/animation/`.
2. Add a variant to `EffectKind` in `src/config/schema.rs` and register it in `create_effect()` (`src/animation/mod.rs`).
