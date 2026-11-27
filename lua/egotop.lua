-- ~/.config/egotop/egotop.lua — Primary EgoTop configuration & composition script.
-- Every visual element (themes, backgrounds, effects, animations, fonts, and layout)
-- is configured here using the public `egotop` Lua API.

local matrix_theme = require("themes.matrix")
local matrix_fx = require("effects.matrix")

-- 1. General Settings
egotop.config({
    name = "egotop",
    greeting = "Hello, {user}!",
    message = "Welcome back to your Linux environment.",
    refresh_rate_ms = 1000,
    animation_fps = 30,
    time_format = "%H:%M:%S",
    date_format = "%m/%d/%Y",
    view_mode = "dashboard",
    default_page = 1,
    startup_animation = "matrix_boot",
    font = "ANSI Shadow",
    use_nerd_fonts = true,
})

-- 2. Optional Wait-for-Input Gate (replaces legacy [init]; omit or leave disabled to launch directly)
-- egotop.wait({
--     trigger = "any",          -- "key", "mouse", or "any"
--     message = "Press any key to start",
--     cursor = { character = "▌", color = "#39FF88", blink = true, blink_interval_ms = 500 },
--     animation = "cursor_expand",
--     after = "reveal",
-- })

-- 3. Theme (importable module with optional local overrides)
egotop.theme({
    theme = matrix_theme.setup,
    -- Example local override:
    -- background = "transparent",
    -- panel_background = "transparent",
})

-- 4. Background (base layer + independent visual effects)
egotop.background({
    base = {
        type = "theme", -- "theme", "transparent", "solid", or "gradient"
        opacity = 1.0,
        gradient = {
            from = "#050A07",
            to = "#08110C",
            direction = "vertical",
        },
    },
    effects = {
        matrix_fx.setup({
            speed = 1.0,
            density = 0.60,
        }),
    },
})

-- 5. Footer & Status Indicator
egotop.footer({
    enabled = true,
    text = "Monitoring system telemetry",
    icon = "activity",
    color = "#39FF88",
    align = "left",
    animation = "spinner",
    fps = 12,
})

-- 6. Explicit Container & Component Tree (no hidden hardcoded Rust tree)
egotop.layout({
    id = "root",
    direction = "vertical",
    width = "100%",
    height = "100%",
    padding = 1,
    gap = 1,
    transparent = true,
    children = {
        egotop.component({
            id = "header",
            kind = "header",
            category = "dynamic",
            width = "fill",
            height = 1,
        }),
        egotop.component({
            id = "greeting",
            kind = "greeting",
            category = "static",
            width = "fill",
            height = 10,
            min_height = 6,
            align = "center",
            font = "ANSI Shadow",
            border = { enabled = true, style = "rounded", title = "Welcome" },
        }),
        egotop.container({
            id = "middle_row",
            direction = "horizontal",
            width = "fill",
            height = "fill",
            min_height = 6,
            gap = 1,
            transparent = true,
            children = {
                egotop.component({
                    id = "system",
                    kind = "system",
                    category = "dynamic",
                    width = "50%",
                    height = "fill",
                    padding = 1,
                    border = { enabled = true, style = "rounded", title = "System" },
                }),
                egotop.component({
                    id = "hardware",
                    kind = "hardware",
                    category = "dynamic",
                    width = "50%",
                    height = "fill",
                    padding = 1,
                    graph_style = "bar",
                    border = { enabled = true, style = "rounded", title = "Hardware" },
                }),
            },
        }),
        egotop.container({
            id = "info_row",
            direction = "horizontal",
            width = "fill",
            height = "fill",
            min_height = 5,
            gap = 1,
            transparent = true,
            children = {
                egotop.component({
                    id = "environment",
                    kind = "environment",
                    category = "dynamic",
                    width = "40%",
                    height = "fill",
                    padding = 1,
                    border = { enabled = true, style = "rounded", title = "Environment" },
                }),
                egotop.component({
                    id = "terminal",
                    kind = "terminal",
                    category = "dynamic",
                    width = "30%",
                    height = "fill",
                    padding = 1,
                    border = { enabled = true, style = "rounded", title = "Terminal" },
                }),
                egotop.component({
                    id = "storage",
                    kind = "storage",
                    category = "dynamic",
                    width = "fill",
                    height = "fill",
                    padding = 1,
                    border = { enabled = true, style = "rounded", title = "Storage" },
                }),
            },
        }),
        egotop.container({
            id = "bottom_row",
            direction = "horizontal",
            width = "fill",
            height = "fill",
            min_height = 6,
            gap = 1,
            transparent = true,
            children = {
                egotop.component({
                    id = "activity",
                    kind = "activity",
                    category = "dynamic",
                    width = "40%",
                    height = "fill",
                    padding = 1,
                    graph_style = "sparkline",
                    border = { enabled = true, style = "rounded", title = "Activity" },
                }),
                egotop.component({
                    id = "network",
                    kind = "network",
                    category = "dynamic",
                    width = "30%",
                    height = "fill",
                    padding = 1,
                    border = { enabled = true, style = "rounded", title = "Network" },
                }),
                egotop.component({
                    id = "session",
                    kind = "session",
                    category = "dynamic",
                    width = "fill",
                    height = "fill",
                    padding = 1,
                    border = { enabled = true, style = "rounded", title = "Session & Uptime" },
                }),
            },
        }),
        egotop.component({
            id = "status_spinner",
            kind = "spinner",
            category = "animated",
            width = "fill",
            height = 1,
            text = "Monitoring system telemetry",
            animation = { fps = 12, speed = 1.0, style = "braille" },
        }),
    },
})
