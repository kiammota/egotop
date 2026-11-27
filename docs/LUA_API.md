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

### Custom Effect Module Contract (`effects/*.lua`)

Every effect returns a controller table from `setup(opts)`:

```lua
local fx = {}

function fx.setup(opts)
    return {
        name = "custom_fx",
        fps = opts.fps or 24,
        options = opts,
        init = function(state, ctx) end,
        resize = function(state, ctx) end,
        update = function(state, dt, ctx) end,
        render = function(state, draw, ctx)
            draw:cell(1, 1, "✦", ctx.theme.primary, nil)
            draw:text(4, 2, "hello", ctx.theme.accent, nil)
            draw:fill(0, 0, ctx.width, 1, "─", ctx.theme.muted, nil)
            local c = draw:blend(ctx.theme.primary, ctx.theme.secondary, 0.5)
        end,
    }
end

return fx
```

---

## 5. Layout, Containers & Components: `egotop.layout(tbl)`

Declare the full container tree explicitly with `egotop.layout`, `egotop.container`, and `egotop.component`:

- **Sizes**: integer (`10`), percentage (`"50%"`), or `"fill"`.
- **Container properties**: `id`, `direction` (`"vertical" | "horizontal"`), `width`, `height`, `min_width`, `max_width`, `min_height`, `max_height`, `padding`, `gap`, `transparent`, `background`, `opacity`, `gradient`, `border = { enabled, style, color, title }`, `children`.
- **Component kinds**: `"header"`, `"greeting"`, `"system"`, `"hardware"`, `"environment"`, `"terminal"`, `"storage"`, `"activity"`, `"network"`, `"session"`, `"cpu"`, `"memory"`, `"temperature"`, `"processes"`, `"uptime"`, `"graph"`, `"ascii_logo"`, `"separator"`, `"spinner"`, `"typewriter"`, `"frames"`, `"matrix"`, `"particles"`, `"ascii_object"`, `"text"`.

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
