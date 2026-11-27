# EgoTop Lua API Reference

All configuration, visual styling, effects, animations, fonts, and flat container layouts in **EgoTop** are written in **Lua 5.4** under `~/.config/egotop/`.

---

## Directory Structure

```text
~/.config/egotop/
├── egotop.lua          # Main entrypoint and composition script
├── themes/             # Importable theme modules (`setup(overrides) -> table`)
├── effects/            # Background & container canvas effect modules (`setup(opts) -> effect`)
├── animations/         # Isolated startup / boxed animations (`setup(api)`)
└── fonts/              # ASCII / block font modules (`setup() -> font`)
```

---

## 1. General Configuration: `egotop.config(tbl)`

```lua
egotop.config({
    name = "egotop",
    greeting = "Hello, {user}!",
    user_override = nil,
    message = "Welcome back to your Linux environment.",
    refresh_rate_ms = 1000,
    animation_fps = 30,
    time_format = "%H:%M:%S",
    date_format = "%m/%d/%Y",
    hide_unavailable = false,
    view_mode = "dashboard", -- "dashboard" | "compact" | "minimal"
    default_page = 1,
    startup_animation = "matrix_boot",
    font = "ANSI Shadow",
    ascii_fallback = false,
    use_nerd_fonts = true,
    marquee_speed = 12.0,
})
```

---

## 2. Wait Gate: `egotop.wait(tbl)` (Optional)

```lua
egotop.wait({
    enabled = true,
    trigger = "any", -- "key" | "mouse" | "any"
    message = "Press any key to start",
    animation = "cursor_expand",
    cursor = {
        enabled = true,
        character = "▌",
        color = "#39FF88",
        blink = true,
        blink_interval_ms = 500,
    },
})
```

---

## 3. Themes API: `egotop.theme(tbl)`

```lua
local bamboo = require("themes.bamboo")

egotop.theme({
    theme = bamboo.setup,
    background = "transparent",
    panel_background = "transparent",
    border_type = "rounded", -- "rounded" | "plain" | "double" | "thick" | "none"
})
```

---

## 4. Independent Background Layer: `egotop.background(tbl)`

The background is **not a container** and does not participate in container layout. It renders on its own dedicated background canvas behind containers:

```lua
egotop.background({
    base = {
        type = "gradient", -- "theme" | "transparent" | "solid" | "gradient"
        opacity = 1.0,
        gradient = {
            from = "#050A07",
            to = "#08110C",
            direction = "vertical",
        },
    },
    effects = {
        require("effects.matrix").setup({
            speed = 1.0,
            density = 0.6,
        }),
    },
})
```

---

## 5. Flat Containers & Automatic Layout: `egotop.layout(tbl)`

Containers are **independent, non-nestable units** (`children` is forbidden). Each container has `name`, `description`, an exclusive isolated canvas, optional layout hints (`region`, `order`, `row`, `width`, `height`), and visual/content settings.

Egotop distributes containers left-to-right in a linear queue, automatically computing dimensions, balancing rows on resize, and preventing overlap:

```lua
egotop.layout({
    padding = 1,
    gap = 1,
    min_container_width = 22,
    containers = {
        egotop.container({
            name = "header",
            description = "Top status bar",
            region = "top",
            kind = "header",
            width = "100%",
            height = 1,
            transparent = true,
        }),
        egotop.container({
            name = "system",
            description = "System overview",
            region = "main", -- "top" | "left" | "main" | "right" | "bottom"
            kind = "system",
            border = { enabled = true, style = "rounded", title = "System" },
        }),
        egotop.container({
            name = "tree_panel",
            description = "Procedural 3D Christmas tree canvas",
            region = "main",
            kind = "canvas",
            script = "effects.christmas_tree",
            border = { enabled = true, style = "rounded", title = "Christmas" },
        }),
    },
})
```

### Inline Isolated Lua Canvas Containers

A container can define `init(ctx)`, `update(dt, t, ctx)`, `resize(ctx)`, and `render(canvas, ctx)` directly. Each inline script runs in an **isolated Lua VM** bound exclusively to that container's `canvas`:

```lua
egotop.container({
    name = "custom_widget",
    description = "Self-contained Lua canvas widget",
    region = "main",
    border = { enabled = true, style = "rounded", title = "Custom" },
    render = function(canvas, ctx)
        canvas:clear(" ", nil, nil)
        canvas:text(0, 0, canvas.name .. " (" .. canvas.width .. "x" .. canvas.height .. ")", ctx.theme.primary)
        canvas:text(0, 1, canvas.description, ctx.theme.muted)
    end,
})
```

---

## 6. Isolated Canvas API (`canvas` & `ctx`)

Every rendering unit (each container, the background, and each animation) receives an isolated `LuaCanvasHandle` bound strictly to its own `CanvasBuffer`:

- **Identity & Bounds (Read-Only)**:
  - `canvas.name` — owning unit's `name`
  - `canvas.description` — owning unit's `description`
  - `canvas.width`, `canvas.height` — current local dimensions (updated automatically on terminal resize)
- **Clipped Drawing Methods** (local coordinates `[0..width, 0..height)`, never global):
  - `canvas:clear(ch, fg, bg)`
  - `canvas:put(x, y, ch, fg, bg, intensity)`
  - `canvas:put_z(x, y, z, ch, fg, bg, intensity)`
  - `canvas:text(x, y, str, fg, bg, intensity)`
  - `canvas:ascii(x, y, str_or_lines, fg, bg, intensity)`
  - `canvas:fill(x, y, w, h, ch, fg, bg)`
  - `canvas:blend(c1, c2, t)`
- **Context (`ctx`)**:
  - `ctx.name`, `ctx.description`, `ctx.width`, `ctx.height`, `ctx.dt`, `ctx.t`, `ctx.fps`, `ctx.state`, `ctx.theme`, `ctx.math` (`sin`, `cos`, `clamp`, `lerp`, `smoothstep`, `hsv_to_rgb`, `rgb_scale`, `rgb_lerp`, `rgb_hex`, `noise`, `hash`).
  - There is **no** `ctx.screen`, global canvas registry, or cross-container access.

---

## 7. Footer API: `egotop.footer(tbl)`

```lua
egotop.footer({
    enabled = true,
    text = "Monitoring system telemetry",
    icon = "activity",
    color = "#39FF88",
    align = "left",
    animation = "spinner",
    fps = 12,
})
```

---

## 8. Isolated Animations API (`animations/*.lua`)

Animations receive `setup(api)` and return a module table with `init(ctx)`, `update(dt, t, ctx)`, `resize(ctx)`, and `render(canvas, ctx)` (plus optional `layer = "background" | "boxed" | "overlay"`, `title`, `fps`, `duration`):

```lua
local animation = {}

function animation.setup(api)
    api.ui.reveal(0.0)
    return {
        name = "my_boot",
        description = "Isolated startup animation",
        layer = "overlay",
        fps = 30,
        duration = 1.0,
        render = function(canvas, ctx)
            local p = math.min(1.0, ctx.t / 1.0)
            ctx.reveal(p)
            canvas:text(0, math.floor(canvas.height / 2), ("─"):rep(canvas.width), ctx.theme.primary)
        end,
    }
end

return animation
```
