# Configuration

All Egotop configuration files live under `~/.config/egotop/` (resolved via `$XDG_CONFIG_HOME/egotop`, falling back to `~/.config/egotop`).

## Directory Structure

```text
~/.config/egotop/
├── egotop.toml         # Primary application, startup, theme, typography, background, layout, and widget config
├── window.toml         # Main window geometry, anchor, border behavior, background mode, and padding
├── themes/             # User and shared colorscheme files (*.toml)
├── animations/         # Declarative community animations (*.toml, schema_version = 1)
├── fonts/              # Custom FIGlet font files (*.flf)
└── layouts/            # Layout preset files (*.toml, discovered for layout autocomplete)
```

On first launch without `--config`, Egotop creates these directories automatically, writes the default `egotop.toml`, `window.toml`, and bundled example animations (`cyber_boot.toml`, `neon_curtains.toml`), and never modifies your shell or compositor files.

## Primary Configuration (`~/.config/egotop/egotop.toml`)

Every section uses TOML `#[serde(default)]` semantics: omitted fields fall back to their documented defaults.

### `[general]`

| Key | Type | Default | Allowed Values / Constraints | Description |
|---|---|---|---|---|
| `name` | string | `"egotop"` | Any string | Program name displayed in the header and window title |
| `greeting` | string | `"Hello, {user}!"` | Supports `{user}`, `{host}`, `{distro}`, `{wm}` | Main greeting heading |
| `user_override` | string (optional) | unset | Any string | Overrides the detected OS username |
| `message` | string | `"Welcome back to your Linux environment."` | Any string | Subtitle / custom message |
| `refresh_rate_ms` | integer | `1000` | `>= 100` (alias: `refresh_rate`) | System metric polling interval in milliseconds |
| `animation_fps` | integer | `30` | `1..=120` | Frame rate cap for animations and UI rendering |
| `time_format` | string | `"%H:%M:%S"` | `chrono::strftime` format string | Clock format in header and session panel |
| `date_format` | string | `"%m/%d/%Y"` | `chrono::strftime` format string | Date format in header |
| `hide_unavailable` | boolean | `false` | `true` \| `false` | Hide unavailable hardware/OS fields instead of showing `N/A` |
| `view_mode` | string | `"dashboard"` | `"dashboard"` \| `"compact"` \| `"minimal"` | Initial presentation density |
| `default_page` | integer | `1` | `1..=3` | Initial active page |
| `startup_animation` | string | `"matrix_boot"` | Native ID or community animation name | Startup transition animation in normal mode |
| `startup_duration_ms` | integer | `1100` | `>= 200` recommended | Duration in milliseconds for native startup transitions |

### `[startup]` and `[startup.cursor]`

| Key | Type | Default | Allowed Values | Description |
|---|---|---|---|---|
| `startup.mode` | string | `"normal"` | `"normal"` \| `"cursor"` | Startup presentation mode (`--startup` overrides this) |
| `startup.animation` | string | `"cursor-expand"` | Any native or community animation ID | Transition played when leaving `cursor` mode |
| `startup.trigger` | string | `"mouse"` | `"mouse"` \| `"key"` \| `"any"` | Primary trigger (`Enter` / `Space` are also accepted) |
| `startup.cursor.character` | string | `"▌"` | Single character | Cursor glyph rendered while waiting |
| `startup.cursor.blink` | boolean | `true` | `true` \| `false` | Enables cursor blinking |
| `startup.cursor.blink_interval` | integer | `500` | `>= 50` ms | Blink half-period in milliseconds |
| `startup.cursor.color` | string | `"#39FF88"` | `#RRGGBB`, named color, or `"reset"` | Cursor color |

### `[theme]` and `[theme.borders]`

| Key | Type | Default | Allowed Values | Description |
|---|---|---|---|---|
| `name` | string | `"matrix"` | Built-in or `~/.config/egotop/themes/*.toml` name | Active colorscheme |
| `background` | string | `"#050A07"` | `#RRGGBB`, named color, `"reset"`, or `""` | Canvas background (`"reset"` inherits terminal transparency) |
| `panel_background` | string | `"#08110C"` | `#RRGGBB`, named color, `"reset"`, or `""` | Panel fill color |
| `primary` | string | `"#39FF88"` | `#RRGGBB`, named color, `"reset"`, or `""` | Primary accent color |
| `secondary` | string | `"#00D9FF"` | `#RRGGBB`, named color, `"reset"`, or `""` | Secondary accent color |
| `accent` | string | `"#FFC857"` | `#RRGGBB`, named color, `"reset"`, or `""` | Highlight/warning accent |
| `text` | string | `"#E5E5E5"` | `#RRGGBB`, named color, `"reset"`, or `""` | Standard text color |
| `muted` | string | `"#5F7D6B"` | `#RRGGBB`, named color, `"reset"`, or `""` | Secondary/label muted text |
| `border` | string | `"#1F7A4C"` | `#RRGGBB`, named color, `"reset"`, or `""` | Default border color |
| `danger` | string | `"#FF5555"` | `#RRGGBB`, named color, `"reset"`, or `""` | High-load / danger indicator |
| `border_type` | string | `"rounded"` | `"rounded"` \| `"plain"` (`"solid"`) \| `"double"` \| `"thick"` \| `"ascii"` | Base border character preset |
| `borders.*` | optional string | unset | Single-width (`width == 1`) non-control char | Individual border overrides (see [borders.md](borders.md)) |

### `[typography]`

| Key | Type | Default | Allowed Values | Description |
|---|---|---|---|---|
| `font` | string | `"ANSI Shadow"` | `"ANSI Shadow"`, `"Standard"`, `"Big"`, `"Slant"`, `"Small"`, `"Block"`, `"Doom"`, or `.flf` name/path | Heading ASCII font |
| `logo_font` | string | `"Standard"` | Same as `font` | Secondary logo font |
| `use_nerd_fonts` | boolean | `true` | `true` \| `false` | Enables Nerd Font icons in panels |
| `ascii_fallback` | boolean | `false` | `true` \| `false` | Forces pure ASCII icons and ASCII heading fonts |
| `logo_preset` | string | `"auto"` | `"auto"` \| `"arch"` \| `"hyprland"` \| `"linux"` \| `"egotop"` \| `"none"` | Built-in ASCII logo selection |
| `custom_logo` | string (optional) | unset | Multi-line string | Inline custom ASCII logo |
| `custom_logo_file` | string (optional) | unset | File path (`~/` supported) | Loads custom ASCII logo from file |

### `[header]`

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `true` | Shows header bar inside the main window |
| `show_clock` | boolean | `true` | Shows live time |
| `show_date` | boolean | `true` | Shows current date |
| `show_indicators` | boolean | `true` | Shows active effect, page, and view mode badges |
| `show_fps` | boolean | `false` | Shows smoothed render FPS |

### `[background]` and Effect Subtables

| Key | Type | Default | Allowed Values | Description |
|---|---|---|---|---|
| `effect` | string | `"matrix"` | `"matrix"` \| `"water"` \| `"particles"` \| `"scanlines"` \| `"none"` | Active ambient background effect |
| `enabled` | boolean | `true` | `true` \| `false` | Master toggle for background effects |
| `speed` | float | `1.0` | `0.0..=10.0` | Global speed multiplier |
| `density` | float | `0.60` | `0.0..=1.0` | Global density multiplier |
| `opacity` | float | `0.38` | `0.0..=1.0` | Color blend factor against `theme.background` |
| `region` | table (optional) | unset | `{ x, y, width, height, unit = "percent" \| "cells" }` | Optional screen clipping region |

Subtables:
- `[background.matrix]`: `enabled` (`true`), `speed` (`1.0`), `density` (`0.60`), `min_length` (`5`, `>= 1` and `<= max_length`), `max_length` (`18`), `color` (`"#1F7A4C"`), `head_color` (`"#39FF88"`), `charset` (`"katakana"` \| `"hex"` \| `"ascii"` \| `"binary"`), `region` (optional).
- `[background.water]`: `enabled` (`false`), `speed` (`1.0`), `amplitude` (`0.6`), `density` (`0.7`), `color` (`"#0088AA"`), `crest_color` (`"#00D9FF"`), `region` (optional).
- `[background.particles]`: `enabled` (`false`), `speed` (`0.9`), `density` (`0.5`), `count` (`45`), `color` (`"#39FF88"`), `chars` (`"·•✦∘*+"`), `region` (optional).
- `[background.scanlines]`: `enabled` (`false`), `speed` (`1.0`), `spacing` (`2`, `>= 1`), `intensity` (`0.35`), `color` (`"#164A2E"`), `region` (optional).

### `[layout]` and `[[widgets]]`

- `[layout]`: `mode` (`"responsive"` \| `"free"` \| `"compact"`), `padding` (`1`), `gap` (`1`), `min_width_for_split` (`88`).
- `[[widgets]]`: array of panel definitions with `id` (`"greeting"`, `"logo"`, `"system"`, `"hardware"`, `"activity"`, `"session"`, `"uptime"`, `"message"`), `enabled`, `title`, `position` (`"top-left"`, `"top-center"`, `"top-right"`, `"center-left"`, `"center"`, `"center-right"`, `"bottom-left"`, `"bottom-center"`, `"bottom-right"`, `"absolute"`), `page`, `x`, `y`, `offset_x`, `offset_y`, `width` (integer cells or `"50%"`), `height`, `align` (`"left"` \| `"center"` \| `"right"`), `padding`, `z_index`, `font`, `color`, `border_color`, `show_border`, `fields` (optional array of field names for `system`), `graph_style` (`"sparkline"` \| `"bar"`), `reveal_animation` (`bool`).

## Window Configuration (`~/.config/egotop/window.toml`)

Controls the main window independently of widget contents:

```toml
id = "main"
position = "relative"       # "relative" | "absolute"
anchor = "center"           # "top-left" | "top-center" | "top-right" | "center-left" | "center" | "center-right" | "bottom-left" | "bottom-center" | "bottom-right"
x = 0
y = 0
offset_x = 0
offset_y = 0
width = "88%"               # integer cells or percentage string
height = "86%"
min_width = 36
min_height = 12
draggable = true
resizable = true
padding = 1
visible = true
z_index = 10

[border]
enabled = true
style = "rounded"           # "rounded" | "plain" | "double" | "thick"
title = "egotop"

[background]
mode = "solid"              # "solid" | "transparent"
```

Note on `background.mode = "transparent"`: this sets terminal cell background to `Color::Reset` so your terminal emulator's background/transparency shows through. It does not configure OS compositor transparency.

## Temporary vs. Persistent Changes

- **Runtime changes** made via `:colorscheme`, `:set border.*`, `:set background`, `:window ...`, `:layout ...`, or `:animation set` apply **immediately in memory** without overwriting your TOML files.
- **Persisting changes** requires an explicit command or action:
  - `:write` or `:w` (or `:write all`) writes both `~/.config/egotop/egotop.toml` and `~/.config/egotop/window.toml`.
  - `:write config` / `:write theme` writes `egotop.toml`.
  - `:write window` writes `window.toml`.
  - Pressing `s` inside the visual customizer (`F2` / `[⚙]`) saves the active tab's file.
- **Reloading from disk**: press `c` or run `:reload` to discard unsaved runtime changes and reload all files under `~/.config/egotop/`.
- **Validation**: run `egotop --check-config` to validate TOML syntax and constraints before launching.
