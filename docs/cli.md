# Command-Line Interface (CLI)

Egotop provides a modular administrative CLI built with `clap` (`derive`) under [`src/cli/`](../src/cli/mod.rs), while preserving direct launching (`egotop`) and all existing startup options.

---

## Global Usage

```bash
egotop [OPTIONS] [COMMAND]
```

### Default TUI Launch & Options

| Flag / Option | Description |
|---|---|
| `egotop` | Launches the interactive terminal dashboard (creates default config on first run) |
| `-c, --config <PATH>` | Uses an alternate TOML configuration path (global; also supported with `config` and `doctor`) |
| `-e, --effect <EFFECT>` | Session override for background effect (`matrix`, `water`, `particles`, `scanlines`, `none`) |
| `-p, --page <1..=3>` | Initial dashboard page (`1`, `2`, or `3`) |
| `--startup <MODE>` | Startup mode (`normal` or `cursor`) |
| `--check-config` | Legacy alias equivalent to `egotop config validate` |
| `--print-default-config` | Prints the built-in default `egotop.toml` template to stdout |
| `-h, --help` | Shows help for top-level CLI or any subcommand |
| `-V, --version` | Shows version information |

---

## Administrative Subcommands

### `egotop config`

Inspect, locate, print, or validate Egotop's configuration without opening the TUI.

| Subcommand | Behavior | Exit Code |
|---|---|---|
| `egotop config` | Prints a structured summary (resolved path, theme, border style, layout/view modes, animations, fonts, active widgets). Falls back to defaults if the file has not been created yet. | `0` (or `1` if invalid) |
| `egotop config show` | Prints the loaded configuration serialized as pretty TOML. | `0` (or `1` if invalid) |
| `egotop config path` | Prints the resolved configuration file path (`~/.config/egotop/egotop.toml` or `--config <PATH>`). | `0` |
| `egotop config validate` | Validates syntax, schema, and semantic rules using the shared 3-stage `ConfigurationRuntime` pipeline (`src/runtime/pipeline.rs`) without duplicating validation logic. | `0` if valid; `1` if missing or invalid |

Examples:
```bash
egotop config
egotop config path
egotop config show
egotop config validate
egotop --config /path/to/custom.toml config validate
```

When `egotop config validate` finds errors, detailed diagnostics (code, file, line, column, field, expected/actual values, and hints) are printed to `stderr`.

---

### `egotop fonts list`

Lists all built-in ASCII/FIGlet fonts (`available_fonts()`) together with any custom `.flf` fonts found under `~/.config/egotop/fonts/`, reporting whether each `.flf` file parses cleanly.

```bash
egotop fonts list
```

---

### `egotop context list`

Queries declared runtime contexts. In the current version of Egotop, named runtime contexts are not implemented yet, so this command clearly reports unavailability without fabricating state.

```bash
egotop context list
```

---

### `egotop doctor`

Runs installation, configuration, resource, and environment diagnostics using Egotop's core subsystems:
- Config root directory presence (`~/.config/egotop`)
- Main config validation (`egotop.toml` via shared pipeline)
- Window config presence (`window.toml`)
- Available built-in & custom themes (`load_all_themes`)
- Available bundled & community animations (`AnimationCatalog::load`)
- Built-in & custom `.flf` fonts (`available_fonts` + header checks)
- Discovered layout presets (`load_layout_names`)
- Linux hardware/OS/WM probe (`SystemCollector`)
- Terminal dimensions & truecolor capability (`terminal_supports_truecolor`)

```bash
egotop doctor
```

Exits with code `0` when all critical checks pass (warnings do not fail `doctor`), or `1` when any `[FAIL]` check occurs (e.g., an invalid configuration or missing explicit `--config` path).

---

## Module Architecture (`src/cli/`)

| File | Responsibility |
|---|---|
| [`src/cli/mod.rs`](../src/cli/mod.rs) | Top-level `Cli` parser, `Commands` enum, global `--config` option, TUI launcher, and exit code dispatch |
| [`src/cli/config.rs`](../src/cli/config.rs) | `config` (`show`, `path`, `validate`, summary) delegating directly to `src/config` and `src/runtime/pipeline.rs` |
| [`src/cli/fonts.rs`](../src/cli/fonts.rs) | `fonts list` inspecting `src/ui/fonts.rs` and `~/.config/egotop/fonts/*.flf` |
| [`src/cli/context.rs`](../src/cli/context.rs) | `context list` reporting feature availability clearly |
| [`src/cli/doctor.rs`](../src/cli/doctor.rs) | `doctor` health diagnostics across paths, resources, system collector, and terminal capabilities |
