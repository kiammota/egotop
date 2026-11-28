# egotop

An interactive, Lua-scriptable animated terminal dashboard for Linux & Hyprland written in Rust.

**egotop** pairs a low-level Rust terminal/layout/scheduling engine with a full **Lua 5.4 scripting architecture**: configuration, explicit component layout, importable themes, independent intro/background/canvas animation slots, ASCII fonts, and programmable multi-scene animations live in clean Lua modules under `~/.config/egotop/`.

---

## Demonstration

- Demonstration video: [`assets/demo.mp4`](assets/demo.mp4)
- ![egotop terminal dashboard](assets/screenshot.png)

---

## Key Features

- **Pure Lua Configuration & Composition (`egotop.lua`)**: Configure settings (`egotop.config`), optional input wait gate (`egotop.wait`), themes (`egotop.theme`), background slot (`egotop.background`), footer (`egotop.footer`), and explicit container/component hierarchy (`egotop.layout`).
- **Modular Built-ins Written in Lua**: All built-in themes (`lua/themes/*.lua`), ambient background/canvas effects (`lua/effects/*.lua`), animations (`lua/animations/*.lua`), and ASCII fonts (`lua/fonts/*.lua`) use the exact same public APIs available to users.
- **Shader-Style Lua Canvas & Z-Buffer Engine**: Build truecolor procedural ASCII art, 3D rotating projections, particle systems, and custom panels using `init(ctx)` / `update(dt, t, ctx)` / `render(canvas, ctx)` (`canvas:clear`, `canvas:put`, `canvas:put_z`, `canvas:text`, `egotop.math.*`, plus strict per-container clipping and layer isolation — exemplified by `lua/effects/christmas_tree.lua` and `lua/animations/christmas.lua`).
- **Independent Animation Slots**: Intros play once, backgrounds are opt-in loops behind panels, and canvases draw inside isolated container bounds. No legacy global `fx` runs on startup.
- **Native Linux Telemetry**: OS, motherboard host, kernel, uptime, packages (`pacman`, `flatpak`), CPU/RAM/Swap/Disks/GPU/VRAM, environment/display/WM, terminal info, and live sparklines.
- **Neovim-style Command Mode (`:`)**: Live preview on `Tab` / `Shift+Tab`, `Esc` state restoration, `:colorscheme`, `:intro`, `:background`, `:canvas`, `:layout`, `:container`, `:set`, `:check`, and `:write`.
- **Separated animation slots**: `intro` plays once at startup and can be skipped with `Esc`, `Space`, or `Enter` (`--no-intro` disables it); `background` loops behind the dashboard; `canvas` runs clipped inside a named panel. Lua script modules declare `animation.type = "intro"`, `effect.type = "background"`, or `tree.type = "canvas"`. Use `:intro set <name>`, `:intro replay|off`, `:background set <name>|off|pause|resume|list`, and `:canvas <container_id> <name|off|clear|pause|resume>`.
- **Contextual command completion**: Command descriptors register names, aliases, usage, and completion providers. Tab and Shift+Tab cycle ranked fuzzy matches; candidate lists are resolved from the current argument position and the relevant theme, layout, container, or typed animation registry. For example, `:border ` offers styles and `:border double ` offers container IDs; `:background set ` lists only background scripts and `:intro set ` only intro scripts.
- **Resolution independent canvases**: Canvas userdata exposes exact inner `width`/`height`, physical `aspect` (`width * cell_aspect / height`), configurable `cell_aspect` (default `0.5`, via `canvas:set_cell_aspect(value)`), and `detail`/`ctx.lod` in `[0, 1]`. Lua scripts can map normalized UV coordinates with `canvas:uv(u, v)` and draw using `line_uv`, `circle_uv`, `rect_uv`, `poly_uv`, or `tri_uv`; `dot_uv` rasterizes into an 2×4 Braille grid, and existing `▀`/`▄` `put` calls support independent foreground/background halves. `canvas:shade(x, y, luminance, fg, bg)` selects a density ramp based on panel size, while `canvas:sample(fn, t)` invokes a fragment callback at normalized subpixel positions and rasterizes its booleans/numeric values to Braille. Canvas scripts may implement `on_resize(width, height, ctx)` (or the legacy `resize(ctx)` form); their state and elapsed animation time persist through resizes. See `lua/effects/delorean.lua` for normalized geometry and `lua/effects/christmas_tree.lua` for a 3D scene with adaptive LOD.

---

## Installation

```bash
git clone https://github.com/kiammota/egotop.git
cd egotop
cargo install --path .
```

Run:

```bash
egotop
```

On first launch, `~/.config/egotop/` is initialized with `egotop.lua`, `themes/*.lua`, `effects/*.lua`, `animations/*.lua`, and `fonts/*.lua`.

---

## CLI Commands

```bash
egotop config            # Show configuration summary
egotop config show       # Print current configuration as Lua
egotop config path       # Print path to ~/.config/egotop/egotop.lua
egotop config validate   # Validate Lua configuration syntax and schema
egotop fonts list        # List available Lua & built-in ASCII fonts
egotop doctor            # Run environment diagnostics
egotop --print-default-config
```

---

## Documentation

- [**Complete Lua API Reference**](docs/LUA_API.md) — `egotop.*` API, module contracts, animation lifecycle, fonts, effects, containers, and examples
- [CLI Reference](docs/cli.md)
- [Keybindings](docs/keybindings.md)
- [Command-line & Autocomplete](docs/command-line.md)

---

