# Animations

Egotop has two complementary animation systems inside `src/animation/`:

1. **Startup & Transition Animations** (`src/animation/transition.rs`): reveal/transition effects played over the main window (`TransitionPlayer`).
2. **Ambient Background Effects** (`src/animation/{matrix,water,particles,scanlines,none}.rs`): continuous effects rendered behind windows (`BackgroundEffect`).

## Native Startup & Transition Animations

| ID | Display Name | Description |
|---|---|---|
| `crt_power_on` | CRT Power On | Horizontal CRT beam expanding vertically into the full window |
| `curtains` | Curtains | Symmetric curtains opening from center outward |
| `fade` | Fade | Smooth gradual fade-in to final theme colors |
| `slide` | Slide | Smooth lateral slide-in from the left |
| `matrix_boot` | Matrix Boot | Retro terminal boot sequence resolving scrambled glyphs |
| `cursor-expand` | Cursor Expand | Radial expansion originating from the cursor startup coordinates |
| `none` | None | Instant display with no startup transition |

Preview or select any animation live:
```text
:animation play crt_power_on
:animation set curtains
```
Or open the interactive picker via `:animation` (or `F2` → **Animations**), press `p` to preview, and `Enter` to set as default.

## Community Declarative TOML Animations

Custom animations live in `~/.config/egotop/animations/*.toml` and are loaded at runtime on startup and `:reload` (`src/config/animation_loader.rs`) without recompiling Egotop.

### Schema (`schema_version = 1`)

| Field | Type | Required | Description |
|---|---|---|---|
| `schema_version` | integer (`>= 1`) | Yes | Schema version (`1`) |
| `name` | string | Yes | Unique animation identifier used by `:animation` |
| `display_name` | string | No | Friendly name shown in the picker |
| `author` | string | No | Author metadata |
| `description` | string | No | Short description shown in picker & autocomplete |
| `[[steps]]` | array of tables | Yes (`>= 1`) | Ordered sequence of transition steps |

Each `[[steps]]` entry supports:

| Key | Type | Default | Allowed Values |
|---|---|---|---|
| `effect` | string | `"fade"` | `"crt_line"`, `"curtains"`, `"fade"`, `"slide"`, `"matrix_scramble"`, `"flash"`, `"pause"` (unknown future effects deserialize safely as no-op) |
| `duration_ms` | integer | `400` | Step duration in milliseconds |
| `easing` | string | `"ease_in_out"` | `"linear"`, `"ease_in"`, `"ease_out"`, `"ease_in_out"` |
| `target` | string | `"window"` | `"screen"`, `"window"`, `"content"` |
| `direction` | string | `"center"` | `"left"`, `"right"`, `"top"`, `"bottom"`, `"center"` |
| `color` | string (optional) | theme accent | `#RRGGBB` or named color |
| `chars` | string (optional) | `"01ｱｶｻﾀﾅﾊ#$"` | Character set for `matrix_scramble` |
| `intensity` | float | `1.0` | Step intensity multiplier |

### Complete Example (`~/.config/egotop/animations/cyber_boot.toml`)

```toml
schema_version = 1
name = "cyber_boot"
display_name = "Cyber Boot (Community)"
author = "egotop"
description = "Initial CRT flash followed by Matrix scramble and smooth reveal."

[[steps]]
effect = "crt_line"
duration_ms = 320
easing = "ease_out"
target = "window"
color = "#39FF88"

[[steps]]
effect = "matrix_scramble"
duration_ms = 550
easing = "ease_in_out"
target = "content"
chars = "01ｱｶｻﾀﾅﾊ#$"

[[steps]]
effect = "fade"
duration_ms = 350
easing = "ease_out"
target = "content"
```

To use it on startup, set `startup_animation = "cyber_boot"` under `[general]` in `~/.config/egotop/egotop.toml`, or run `:animation set cyber_boot` followed by `:write`.

## Ambient Background Effects

Configured under `[background]` in `egotop.toml` (or cycled live via `e` or `:effect <name>`):
- `matrix`: vertical falling glyphs (`katakana`, `hex`, `ascii`, `binary`) with configurable `speed`, `density`, `min_length`, `max_length`, `color`, and `head_color`.
- `water`: dual-wave interference surface (`~`, `≈`, `∼`, `·`) with `speed`, `amplitude`, `density`, `color`, and `crest_color`.
- `particles`: independent floating characters (`count`, `speed`, `density`, `color`, `chars`).
- `scanlines`: CRT horizontal scanline bands with moving sweep line (`speed`, `spacing`, `intensity`, `color`).
- `none`: static background.

All background effects support optional `region = { x = 0, y = 0, width = 50, height = 100, unit = "percent" }` to restrict rendering to a screen region.
