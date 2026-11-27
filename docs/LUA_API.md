# EgoTop Lua API Reference

All configuration, visual styling, effects, fonts, layouts, and programmable animations in **EgoTop** are written in **Lua 5.4** under `~/.config/egotop/`.

---

## Directory Structure

```text
~/.config/egotop/
├── egotop.lua          # Main entrypoint and composition script
├── themes/             # Importable theme modules (`setup(overrides) -> table`)
├── effects/            # Background effect modules (`setup(opts) -> effect`)
├── animations/         # Programmable animations (`setup(api)`)
└── fonts/              # ASCII / block font modules (`setup() -> font`)
```

On first launch, EgoTop populates missing built-in modules in `~/.config/egotop/` so you can inspect, copy, or customize any built-in feature using the exact same public APIs. User files on disk always take priority over embedded fallbacks.

---

## 1. General Configuration: `egotop.config(tbl)`

```lua
egotop.config({
    name = "egotop",
    greeting = "Hello, {user}!",
    user_override = nil, -- optional string
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

If omitted, EgoTop opens straight into the dashboard. When declared, it pauses at startup until the configured input trigger fires:

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

Each theme module in `themes/*.lua` exports a `setup(overrides)` function returning a palette and border style:

```lua
local bamboo = require("themes.bamboo")

egotop.theme({
    theme = bamboo.setup,
    -- Local overrides without editing themes/bamboo.lua:
    background = "transparent",
    panel_background = "transparent",
    border_type = "rounded", -- "rounded" | "plain" | "double" | "thick" | "none"
})
```

Supported colors: `"transparent"` / `"reset"`, `#RRGGBB` hex, or named colors (`black`, `white`, `green`, `cyan`, etc.).

---

## 4. Background & Effects API: `egotop.background(tbl)`

The background base (`transparent`, `solid`, or `gradient` with `opacity`) is completely decoupled from visual `effects`:

```lua
egotop.background({
    base = {
        type = "gradient", -- "transparent" | "solid" | "gradient"
        color = "#050A07",
        opacity = 0.9,
        gradient = {
            from = "#050A07",
            to = "#08110C",
            direction = "vertical", -- "horizontal" | "vertical"
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

### Custom Effect & Shader Canvas Contract (`effects/*.lua`)

Every effect and procedural canvas widget implements the structured frame lifecycle returned by `setup(opts)` (or exported directly at top level):

```lua
local fx = {}

function fx.setup(opts)
    return {
        name = "custom_shader",
        fps = opts.fps or 30,
        options = opts,
        init = function(ctx)
            ctx.state.angle = 0.0
        end,
        resize = function(ctx) end,
        update = function(dt, t, ctx)
            ctx.state.angle = t * 0.85
        end,
        render = function(canvas, ctx)
            local x = (canvas.width * 0.5) + ctx.cos(ctx.state.angle) * 6.0
            local y = canvas.height * 0.5
            local col = ctx.hsv_to_rgb((ctx.t * 0.1) % 1.0, 0.85, 1.0)
            canvas:clear(" ", nil, nil)
            canvas:put_z(x, y, 1.0, "@", col, nil, 1.0)
            canvas:text(1, 0, "SHADER", ctx.theme.accent, nil, 1.0)
        end,
    }
end

return fx
```

#### Cell-Buffer `canvas` API & Math Helpers
- **Bounds & Automatic Clipping**: `canvas.width`, `canvas.height` match the exact allocated panel or background viewport; out-of-bounds writes clip safely.
- **Drawing Methods**:
  - `canvas:clear(ch, fg, bg)`
  - `canvas:put(x, y, char, fg_rgb, bg_rgb, intensity)` — supports `#RRGGBB`, `{ r, g, b }`, `nil`/`"transparent"`, and `intensity` scaling.
  - `canvas:put_z(x, y, z, char, fg_rgb, bg_rgb, intensity)` — depth-buffered write (`z >= current_z` wins) for rotating 3D ASCII projections.
  - `canvas:text(x, y, str, fg_rgb, bg_rgb, intensity)`
- **Math & Color Helpers** (`ctx.*`, `ctx.math.*`, and `egotop.math.*`): `sin`, `cos`, `clamp`, `lerp`, `smoothstep`, `hsv_to_rgb`, `rgb_scale`, `rgb_lerp`, `rgb_hex`, `noise`, `hash`.
- **Example Procedural 3D ASCII Shader**: See `lua/effects/christmas_tree.lua` and `lua/animations/christmas.lua` (which uses a 3D conifer foliage mesh with Z-buffered lighting, `@ # % * + : .` density ramp, glowing star, sinusoidal ornaments, gifts, and snowfall).

---

## 5. Layout, Containers & Components: `egotop.layout(tbl)`

Declare the full container tree explicitly with `egotop.layout`, `egotop.container`, and `egotop.component`:

- **Sizes**: integer (`10`), percentage (`"50%"`), or `"fill"`.
- **Container properties**: `id`, `direction` (`"vertical" | "horizontal"`), `width`, `height`, `min_width`, `max_width`, `min_height`, `max_height`, `padding`, `gap`, `transparent`, `background`, `opacity`, `gradient`, `border = { enabled, style, color, title }`, `children`.
- **Component kinds**: `"header"`, `"greeting"`, `"system"`, `"hardware"`, `"environment"`, `"terminal"`, `"storage"`, `"activity"`, `"network"`, `"session"`, `"cpu"`, `"memory"`, `"temperature"`, `"processes"`, `"uptime"`, `"graph"`, `"ascii_logo"`, `"separator"`, `"spinner"`, `"typewriter"`, `"frames"`, `"matrix"`, `"particles"`, `"ascii_object"`, `"canvas"`, `"text"`.
- **Mounting a Lua Canvas Widget** (strictly clipped inside its own bordered panel):
```lua
egotop.component({
    id = "tree_panel",
    kind = "canvas",
    script = "effects.christmas_tree",
    width = "fill",
    height = 18,
    border = { enabled = true, style = "rounded", title = "Christmas" },
})
```

---

## 6. Footer API: `egotop.footer(tbl)`

```lua
egotop.footer({
    enabled = true,
    text = "Monitoring system telemetry",
    icon = "activity", -- "activity" | "dots" | "line" | "blocks" | "pulse" | "none"
    color = "#39FF88",
    align = "left",    -- "left" | "center" | "right"
    animation = "spinner",
    fps = 12,
})
```

---

## 7. Programmable Animations API (`animations/*.lua`)

Every animation module exposes a single entry point: `setup(api)`:

```lua
local animation = {}

function animation.setup(api)
    api.ui.reveal(0.0)
    local elem = api.element({
        id = "star",
        x = math.floor(api.screen.width() / 2),
        y = 3,
        text = "★",
        fg = "#FFE66D",
        bold = true,
    })

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 1200,
        easing = "ease_out_cubic",
        on_update = function(p)
            api.ui.reveal(p)
        end,
        on_complete = function()
            elem:destroy()
            api.finish()
        end,
    })
end

return animation
```

### `api` Reference
- **Screen & Clock**: `api.screen.width()`, `api.screen.height()`, `api.elapsed_ms()`, `api.dt()`, `api.finish()`
- **Scheduling**: `api.after(delay_ms, fn)`, `api.every(interval_ms, fn)`, `api.tween(spec)`, `api.sequence(steps, on_complete)`, `api.parallel(fns)`
- **Elements**: `elem = api.element({ id, x, y, text, lines, fg, bg, bold, visible })` with `:move(x, y)`, `:set_text(s)`, `:set_lines(tbl)`, `:set_color(fg, bg)`, `:set_visible(bool)`, `:destroy()`
- **Text & Colors**: `api.figlet(text, font_name)`, `api.lerp_color(c1, c2, t)`
- **UI Hooks**: `api.ui.reveal(progress)`, `api.ui.set_greeting_transform(function(ch, col, row) return { char = ch, fg = "#...", dy = 0, bold = true } end)`

---

## 8. ASCII Fonts API (`fonts/*.lua`)

```lua
local font = {}

function font.setup()
    return {
        name = "Custom",
        height = 4,
        spacing = 1,
        fallback = "?",
        characters = {
            A = {
                "  ██  ",
                " █  █ ",
                "██████",
                "█    █",
            },
        },
    }
end

return font
```

---

## Error Handling

Lua syntax or runtime errors are isolated safely:
- `egotop --config ~/.config/egotop/egotop.lua config validate` reports exact errors cleanly before launching.
- Effect and animation callbacks catch errors in `last_error` without freezing input handling or crashing the terminal.
