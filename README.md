# egotop

**egotop** is an interactive terminal welcome dashboard written in Rust for Linux, Hyprland, Wayland, and X11.

Run `egotop` with zero required arguments to launch a customizable terminal environment featuring live Linux system metrics, startup and background animations, FIGlet ASCII typography, a Neovim-style command line with contextual autocomplete, runtime border customization, and an interactive cursor startup mode.

---

## Features

- **English-first interface**: All panel titles, notifications, commands, autocomplete descriptions, and error messages are presented in English (while preserving user-supplied custom greetings and quotes).
- **Command-line & Contextual Autocomplete (`:`)**:
  - Hidden until `:` is pressed.
  - Real-time popup powered by the central `CommandRegistry` and `CompletionEngine` (fetching themes, animations, layouts, and border properties dynamically from runtime managers with zero per-keystroke disk reads).
  - Full cursor editing (`Left`, `Right`, `Home`, `End`, `Backspace`, `Delete`), shell-style completion (`Tab` / `Shift+Tab` immediately completes the active token, `Space` advances to argument suggestions, `Enter` executes, `Esc` closes popup or cancels), and command history.
- **Runtime Border Customization**:
  - Centralized `BorderChars` system governing every window, panel, header, modal, and completion popup.
  - Customize individual characters live (`:set border.horizontal ━`, `:set border.top_left ╔`) or presets (`:set border.style rounded|plain|double|thick`) with Unicode display-width validation (`width == 1`).
  - Changes remain temporary in runtime until explicitly persisted with `:write` (or `:w`).
- **Cursor Startup Mode (`--startup cursor`)**:
  - Displays only a blinking cursor on startup and reveals the dashboard via the Animation Engine (`cursor-expand`) originating from the exact mouse click or cursor coordinates.
- **Window Engine**:
  - Independent window state (`~/.config/egotop/window.toml`) with relative anchors, absolute positioning, mouse drag (top border), bottom-right resize handle (`◢`), configurable borders, padding, z-index, and solid or terminal-cell transparent (`Color::Reset`) backgrounds.
- **Native Linux System Metrics**:
  - Reads OS release, kernel, architecture, shell, terminal, DE/WM, CPU delta/temperature, RAM/Swap, mounted disks, GPU/VRAM, load average, processes, threads, and uptime directly from `/proc`, `/sys`, `procfs`, and `libc`.

---

## Configuration (`~/.config/egotop/`)

```text
~/.config/egotop/
├── egotop.toml         # General settings, startup mode, theme, borders, layout, and widgets
├── window.toml         # Main window geometry, anchor, border, background, and behavior
├── themes/             # Shareable colorschemes (*.toml)
├── animations/         # Community declarative animations (*.toml, schema_version = 1)
├── fonts/              # Custom FIGlet fonts (*.flf)
└── layouts/            # Custom layout presets (*.toml)
```

### Custom Borders (`[theme.borders]`)

```toml
[theme.borders]
top_left = "╭"
top_right = "╮"
bottom_left = "╰"
bottom_right = "╯"
horizontal = "─"
vertical = "│"
left_separator = "├"
right_separator = "┤"
top_separator = "┬"
bottom_separator = "┴"
intersection = "┼"
```

### Cursor Startup Mode (`[startup]`)

```toml
[startup]
mode = "cursor"             # "normal" | "cursor"
animation = "cursor-expand" # Any native or community animation
trigger = "mouse"           # "mouse" | "key" | "any"

[startup.cursor]
character = "▌"
blink = true
blink_interval = 500
color = "#39FF88"
```

### Community Declarative Animations (`~/.config/egotop/animations/*.toml`)

```toml
schema_version = 1
name = "cyber_boot"
display_name = "Cyber Boot (Community)"
author = "egotop"
description = "Initial CRT flash followed by Matrix scramble and smooth reveal."

[[steps]]
effect = "crt_line"        # crt_line | curtains | fade | slide | matrix_scramble | flash | pause
duration_ms = 320
easing = "ease_out"        # linear | ease_in | ease_out | ease_in_out
target = "window"
color = "#39FF88"
```

---

## Commands (`:`)

- `:colorscheme <name>` — Apply a colorscheme from built-ins or `~/.config/egotop/themes/`
- `:set border.<prop> <char>` — Set individual border character (`horizontal`, `vertical`, `top_left`, `top_right`, `bottom_left`, `bottom_right`, etc.)
- `:set border.style <style>` — Apply border preset (`rounded`, `plain`, `double`, `thick`)
- `:set background <dark|light>` — Switch background brightness mode in runtime
- `:animation [play|set] <name>` — Preview or set the startup animation
- `:layout [load|mode] <name>` — Switch layout mode or load a preset
- `:window [subcommand]` — Configure anchor, borders, background, drag, resize, padding, or reset
- `:effect [name]` — Switch background ambient effect (`matrix`, `water`, `particles`, `scanlines`, `none`)
- `:write [all|config|window|theme]` / `:w` — Persist runtime changes to `~/.config/egotop/`
- `:reload` — Reload configuration, themes, animations, and window state from disk
- `:help` / `:quit` / `:q` — Open help or quit

---

## Build, CLI, and Shell / Hyprland Integration

```bash
cd ~/projects/egotop
cargo test
cargo build --release
cargo install --path .
```

CLI options:

```bash
egotop                    # Launch with ~/.config/egotop/egotop.toml
egotop --startup cursor   # Launch in cursor startup mode (overrides TOML)
egotop --startup normal   # Launch in normal startup mode
egotop --effect water     # Override background effect
egotop --check-config     # Validate configuration and exit
```

### Zsh (`~/.zshrc`) / Bash (`~/.bashrc`)

```zsh
if [[ -o interactive ]] && [[ -z "$EGOTOP_SHOWN" ]] && command -v egotop >/dev/null 2>&1; then
  export EGOTOP_SHOWN=1
  egotop
fi
```

### Hyprland (`~/.config/hypr/hyprland.conf`)

```ini
windowrulev2 = float, class:^(egotop-welcome)$
windowrulev2 = size 1150 720, class:^(egotop-welcome)$
windowrulev2 = center, class:^(egotop-welcome)$

exec-once = kitty --class egotop-welcome -e egotop
```
