# egotop

An interactive, animated terminal welcome dashboard for Linux and Hyprland written in Rust.

**egotop** combines native Linux system inspection with an independent window engine, startup transitions, ambient background effects, FIGlet ASCII typography, runtime border customization, and a Neovim-style command line with contextual autocomplete.

---

## Demonstration

- Demonstration video: [`assets/demo.mp4`](assets/demo.mp4) (recorded directly from a live `egotop --startup cursor` terminal session)

![egotop terminal dashboard](assets/screenshot.png)

---

## Implemented Features

- **Native Linux System Information**: OS release, kernel, architecture, hostname, user, shell and version, terminal emulator, DE/WM (`Hyprland`, `Sway`, `i3`), Wayland/X11 session, CPU usage & temperature (`/proc/stat`, `hwmon`), RAM & Swap (`/proc/meminfo`), mounted disks (`statvfs`), GPU/VRAM (`/sys/class/drm`), load average, processes, threads, uptime, and real-time CPU/RAM sparklines.
- **Window Engine**: Independent main window (`~/.config/egotop/window.toml`) with relative anchors, absolute coordinates, mouse drag (top border), bottom-right resize handle (`◢`), z-index ordering, configurable borders, padding, and solid or terminal-cell transparent (`Color::Reset`) backgrounds.
- **Command-line & Contextual Autocomplete (`:`)**: Hidden until `:` is pressed; shell-style `Tab` / `Shift+Tab` cycling immediately updates the active token without requiring `Enter`, `Space` advances to argument completions, and `Enter` executes.
- **Runtime Border & Theme Customization**: Switch colorschemes (`:colorscheme <name>`) or customize border styles and individual characters (`:set border.horizontal ━`, `:set border.style double`) immediately in runtime, then persist explicitly with `:write` (`:w`).
- **Startup & Background Animations**: Native startup transitions (`crt_power_on`, `curtains`, `fade`, `slide`, `matrix_boot`, `cursor-expand`), interactive cursor startup mode (`--startup cursor`), declarative community TOML animations (`~/.config/egotop/animations/*.toml`), and ambient background effects (`matrix`, `water`, `particles`, `scanlines`, `none`).
- **ASCII Typography & Logos**: Built-in fonts (`ANSI Shadow`, `Standard`, `Big`, `Slant`, `Small`, `Block`, `Doom`), custom `.flf` FIGlet font loader with horizontal kerning/smushing, built-in and custom ASCII logos, and Nerd Font / pure ASCII fallback support.

---

## Requirements

- Linux (`x86_64` or `aarch64`)
- Rust toolchain (`rustc` / `cargo`, edition 2024; verified with `rustc 1.96.0`)
- Terminal emulator with truecolor support recommended (Kitty, Alacritty, WezTerm, foot)
- Optional: a Nerd Font for icons (set `use_nerd_fonts = false` or `ascii_fallback = true` if unavailable)

---

## Installation

Clone and build from source:

```bash
git clone https://github.com/kiammota/egotop.git
cd egotop
cargo build --release
```

Run directly:

```bash
./target/release/egotop
```

Or install into `~/.cargo/bin`:

```bash
cargo install --path .
```

---

## Quick Start

Launch with defaults (creates `~/.config/egotop/` on first run):

```bash
egotop
```

Launch in interactive cursor startup mode (blinking cursor until click, `Enter`, or `Space`):

```bash
egotop --startup cursor
```

Validate your configuration file without opening the TUI:

```bash
egotop --check-config
```

Try commands inside Egotop by pressing `:`:

```text
:colorscheme nord
:set border.style double
:set border.horizontal ━
:animation play crt_power_on
:window bg transparent
:write
:quit
```

---

## Configuration Files

All configuration files are stored under `~/.config/egotop/`:

- `~/.config/egotop/egotop.toml` — Main settings, startup mode, theme, borders, typography, background effects, layout, and widgets
- `~/.config/egotop/window.toml` — Main window geometry, anchor, border behavior, and background mode
- `~/.config/egotop/themes/*.toml` — Custom and shared colorschemes
- `~/.config/egotop/animations/*.toml` — Declarative community animations
- `~/.config/egotop/fonts/*.flf` — Custom FIGlet fonts

---

## Documentation

Full technical documentation is available in [`docs/`](docs/):

- [Configuration](docs/configuration.md) — All TOML sections, keys, types, defaults, and persistence model
- [Themes](docs/themes.md) — Built-in themes, custom theme TOML schema, validation, and troubleshooting
- [Borders](docs/borders.md) — Centralized border character customization in TOML and runtime
- [Keybindings](docs/keybindings.md) — Normal, command-line, customizer, mouse, and cursor-startup controls
- [Command-line & Autocomplete](docs/command-line.md) — Full command reference and shell-style Tab/Shift+Tab/Space behavior
- [Animations](docs/animations.md) — Native transitions, community declarative TOML schema, and background effects
- [Architecture](docs/architecture.md) — Module layout and extension guide

---

## Contributing & License

- **Contributing:** Issues and pull requests are welcome at [github.com/kiammota/egotop](https://github.com/kiammota/egotop). Run `cargo test` and `cargo clippy --all-targets -- -D warnings` before submitting changes.
- **License:** Declared as `MIT` in [`Cargo.toml`](Cargo.toml).
