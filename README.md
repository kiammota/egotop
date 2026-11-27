# egotop

An interactive, Lua-scriptable animated terminal dashboard for Linux & Hyprland written in Rust.

**egotop** pairs a low-level Rust terminal/layout/scheduling engine with a full **Lua 5.4 scripting architecture**: configuration, explicit component layout, importable themes, decoupled background bases & visual effects, ASCII fonts, and programmable multi-scene animations live in clean Lua modules under `~/.config/egotop/`.

---

## Demonstration

- Demonstration video: [`assets/demo.mp4`](assets/demo.mp4)
- ![egotop terminal dashboard](assets/screenshot.png)

---

## Key Features

- **Pure Lua Configuration & Composition (`egotop.lua`)**: Configure settings (`egotop.config`), optional input wait gate (`egotop.wait`), themes (`egotop.theme`), background base & effect stack (`egotop.background`), footer (`egotop.footer`), and explicit container/component hierarchy (`egotop.layout`).
- **Modular Built-ins Written in Lua**: All built-in themes (`lua/themes/*.lua`), ambient background/canvas effects (`lua/effects/*.lua`), animations (`lua/animations/*.lua`), and ASCII fonts (`lua/fonts/*.lua`) use the exact same public APIs available to users.
- **Shader-Style Lua Canvas & Z-Buffer Engine**: Build truecolor procedural ASCII art, 3D rotating projections, particle systems, and custom panels using `init(ctx)` / `update(dt, t, ctx)` / `render(canvas, ctx)` (`canvas:clear`, `canvas:put`, `canvas:put_z`, `canvas:text`, `egotop.math.*`, plus strict per-container clipping and layer isolation — exemplified by `lua/effects/christmas_tree.lua` and `lua/animations/christmas.lua`).
- **Independent Background & Container Transparency**: Decouple base layers (`transparent`, `solid`, `gradient`, `opacity`) from visual effects (`matrix`, `water`, `particles`, `scanlines`, `starfield`, `noise`, `floating_chars`, `christmas_tree`), and configure containers globally or individually.
- **Native Linux Telemetry**: OS, motherboard host, kernel, uptime, packages (`pacman`, `flatpak`), CPU/RAM/Swap/Disks/GPU/VRAM, environment/display/WM, terminal info, and live sparklines.
- **Neovim-style Command Mode (`:`)**: Live preview on `Tab` / `Shift+Tab`, `Esc` state restoration, `:colorscheme`, `:effect`, `:animation play`, `:layout`, `:container`, `:set`, `:check`, and `:write`.

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

## License

MIT — see [`Cargo.toml`](Cargo.toml).
