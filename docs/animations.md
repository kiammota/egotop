# Animations, Scenes & Backgrounds

Egotop uses a **single unified declarative Animation Engine** (`src/animation/`) across every bundled animation and community package. Bundled presets and external community packages use the exact same TOML schema, parser, validator, scene graph, geometry morphing engine, timeline evaluator, text engine, and multi-layer background pipeline—only their package origin differs (`Bundled` vs. `Community`).

---

## Architecture Overview

| Module | Path | Responsibility |
|---|---|---|
| **Unified Engine & Catalog** | [`src/animation/transition.rs`](../src/animation/transition.rs) | `AnimationCatalog` (bundled + community packages) and `TransitionPlayer` (`TimelineClock`, transition effects, scene rendering) |
| **Scene System & Layers** | [`src/animation/scene.rs`](../src/animation/scene.rs) | Z-ordered `LayerSpec` composition, coordinate anchors (`center`, `%`, `cells`, `cursor`), ASCII sprites, truecolor + ANSI-256 adaptation |
| **Geometry Engine** | [`src/animation/geometry.rs`](../src/animation/geometry.rs) | Primitives (`circle`, `square`, `rectangle`, `triangle`, `polygon`, `line`, `path`), 64-point arc-length contour morphing, `braille` / `quadrant` / `ascii` subcell rasterizers |
| **Timeline & Loop System** | [`src/animation/timeline.rs`](../src/animation/timeline.rs) | Multi-track keyframe interpolation (`position`, `size`, `color`, `rotation`, `scale`, `opacity`, `visibility`, `shape`, `text`) and `once` / `infinite` / `repeat` / `ping_pong` loops |
| **Easing Curves** | [`src/animation/easing.rs`](../src/animation/easing.rs) | `linear`, `ease_in`, `ease_out`, `ease_in_out`, and `cubic_bezier(x1, y1, x2, y2)` |
| **Text Animation Engine** | [`src/animation/text_fx.rs`](../src/animation/text_fx.rs) | `typewriter`, `fade`, `reveal`, `wave`, `glitch`, `color_cycle`, `char_by_char`, `scroll`, `pulse` |
| **Background Pipeline** | [`src/animation/backgrounds.rs`](../src/animation/backgrounds.rs) | Multi-layer composable backgrounds (`matrix_rain`, `starfield`, `particles`, `waves`, `noise`, `gradient`, `floating_chars`, `custom_ascii`, `scanlines`) |
| **Schema & Sandbox Loader** | [`src/config/animation_loader.rs`](../src/config/animation_loader.rs) | v1/v2 TOML parsing, schema & resource limit validation, sandboxed package asset resolution |

---

## Bundled Animations & Showcases

Every bundled animation ships as a declarative TOML manifest (or package folder) inside [`assets/animations/`](../assets/animations/):

### Startup & Window Transitions
- `crt_power_on` — Horizontal CRT beam expanding vertically into the full window.
- `curtains` — Symmetric curtains opening from the center outward.
- `fade` — Smooth fade-in to active theme colors.
- `slide` — Lateral window slide reveal.
- `matrix_boot` — Scrambled glyph column reveal.
- `cursor-expand` — Radial reveal expanding from the cursor startup position.
- `none` — Instant display.
- `cyber_boot` & `neon_curtains` — Multi-step v1 declarative boot sequences.

### Full Scene, Geometry & Multi-Layer Showcases
- `shape-morph` — Ping-pong Braille contour morph (`circle -> square -> triangle -> hexagon -> circle`) with inner rotating core, ASCII frame asset, particle aura, `color_cycle` title, `typewriter` caption, and starfield background.
- `orbital-prism` — Counter-rotating nested polygons, keyframed orbiting beacon, `wave` title, and deep-space starfield.
- `neon-banner` — Demonstrates `color_cycle`, `wave`, `glitch`, and `scroll` text effects with keyframed ASCII emblem over `gradient` + `floating_chars` layers.
- `aurora-horizon` — Multi-layer `gradient` + `noise` + `waves` background with morphing custom `path` aurora curve and `pulse` heading.
- `matrix-overload` — Layered `matrix_rain` + `scanlines` background with rotating octagonal Braille reticle, `core.txt` ASCII reactor, and `reveal` telemetry.
- `starlight-warp` — `starfield` + `floating_chars` with diamond-to-octagon `cubic_bezier` portal morph, radial particles, and `char_by_char` title.
- `pulse-scanner` — `line` morphing to `rectangle` over 4 `repeat` cycles (`interval_ms = 250`) with multi-frame ASCII LED status sprites.

Preview any animation live:
```text
:animation play shape-morph
:animation pause
:animation resume
:animation speed 1.5
:animation stop
```
Or press `F2` → **Animations**, `p` to play, `Enter` to set startup default, or `Esc` to dismiss an active foreground scene.

---

## Declarative TOML Schema (`version = 2`)

Schema `version = 1` (`[[steps]]`) remains 100% backwards-compatible and compiles cleanly into the unified timeline engine. Schema `version = 2` unlocks full scenes, layers, geometry morphing, text effects, particles, loops, and multi-layer backgrounds:

```toml
[animation]
name = "shape-morph"
display_name = "Shape Morph Showcase"
author = "egotop"
description = "Braille circle-to-square-to-triangle morph."
version = 2
duration_ms = 2400
fps = 30
speed = 1.0
mode = "scene"          # "scene" | "transition" | "composite"
target = "screen"       # "screen" | "window" | "content" | "overlay" | "background"

[animation.loop]
mode = "ping_pong"      # "once" | "infinite" | "repeat" | "ping_pong"
count = 0               # cycle limit for "repeat" or finite "ping_pong"
interval_ms = 0         # optional pause between repetitions

[[layers]]
id = "geometry"
z_index = 10
visible = true
opacity = 1.0

[[backgrounds]]
effect = "starfield"    # matrix_rain | starfield | particles | waves | noise | gradient | floating_chars | custom_ascii | scanlines | none
z_index = 0
density = 0.7
speed = 1.0
color = "#1F7A4C"
secondary_color = "#00D9FF"

[[tracks]]
id = "main-shape"
type = "shape"          # shape | text | ascii | particles | transition | background
layer = "geometry"
primitive = "circle"    # circle | square | rectangle | triangle | polygon | line | path
renderer = "braille"    # braille | block | quadrant | ascii
position = ["center", "center"]   # cells, "50%", "center", "left", "right", "top", "bottom", "cursor"
size = [24, 14]
stroke = true
fill = false
color = "#39FF88"

[[tracks.keyframes]]
at_ms = 0
shape = "circle"
color = "#39FF88"
rotation = 0.0
scale = 0.9
easing = "ease_in_out"

[[tracks.keyframes]]
at_ms = 1200
shape = "square"
color = "#00D9FF"
rotation = 45.0
scale = 1.1
transition = "morph"    # "morph" | "snap" | "fade"
easing = "cubic_bezier(0.25, 0.1, 0.25, 1.0)"
```

### Geometry & Subcell Renderers
- **Shapes:** `circle`, `square`, `rectangle`, `triangle`, `polygon` (with `sides = 3..32`), `line`, and `path` (normalized coordinates in `[-1.0, 1.0]`).
- **Morphing:** Every contour is resampled uniformly by arc length to 64 vertices so any primitive smoothly interpolates into any other without jagged character jumping.
- **Renderers:** `braille` (`2×4` Unicode Braille dots per cell, `U+2800..U+28FF`), `quadrant` / `block` (`2×2` subcell blocks), and `ascii` (`stroke_char` / `fill_char`).

### Easing Curves
- `"linear"`
- `"ease_in"`
- `"ease_out"`
- `"ease_in_out"`
- `"cubic_bezier(x1, y1, x2, y2)"` (or table `{ type = "cubic_bezier", points = [x1, y1, x2, y2] }`)

### Text Effects (`type = "text"`)
Supported `text_effect` values: `none`, `typewriter`, `fade`, `reveal`, `wave`, `glitch`, `color_cycle`, `char_by_char`, `scroll`, `pulse`. Configure `direction` (`left`, `right`, `up`, `down`, `center`), `speed`, `intensity`, `palette`, `chars`, and `color`.

---

## Community Package Format & Sandboxing

Place custom animations under `~/.config/egotop/animations/` either as:
1. **A directory bundle** (recommended when bundling ASCII art assets):
   ```text
   ~/.config/egotop/animations/my-scene/
   ├── animation.toml
   ├── assets/
   │   ├── emblem.txt
   │   └── frame.txt
   └── preview.png      # optional preview metadata
   ```
2. **A single `.toml` file** (e.g. `~/.config/egotop/animations/my_anim.toml`), which must be self-contained without external asset paths.

### Validation & Security Limits
Community packages run inside a strict declarative sandbox (no code execution and no arbitrary file access):
- Asset references (`asset = "assets/emblem.txt"`) must be relative, must not contain `..`, `\`, URL schemes, or absolute paths, and are canonicalized and verified to stay inside their package directory (blocking symlink escapes).
- Standalone `.toml` manifests outside a package folder cannot reference external `asset` files.
- Resource bounds enforced at load time:
  - `schema_version` / `version`: `1..=2`
  - `fps`: `1..=120`
  - `duration_ms`: `<= 300_000`
  - `layers`: `<= 16`
  - `tracks`: `<= 32`
  - `backgrounds`: `<= 8`
  - `keyframes` per track: `<= 64`
  - `count` (particles per track): `<= 500`
  - `path` points: `<= 128`
  - Asset file size: `<= 64 KiB` UTF-8 text (`animation.toml` `<= 128 KiB`)

Run `:reload` inside Egotop after editing or installing a package to hot-reload without restarting.

---

## Composable Multi-Layer Backgrounds

Stack or replace ambient backgrounds at runtime using `:background`:
```text
:background set starfield
:background add noise
:background speed 1.4
:background pause
:background resume
:background stop
```
Available background layer algorithms:
- `matrix_rain` (alias `matrix`)
- `starfield`
- `particles`
- `waves` (alias `water`)
- `noise`
- `gradient`
- `floating_chars`
- `custom_ascii` (renders ASCII asset/sprite with gentle drift or scroll)
- `scanlines`
- `none`
