-- ~/.config/egotop/egotop.lua — Primary EgoTop configuration & composition script.
-- Every visual element (themes, backgrounds, effects, animations, fonts, and flat containers)
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

-- 2. Optional Wait-for-Input Gate
-- egotop.wait({
--     trigger = "any",          -- "key", "mouse", or "any"
--     message = "Press any key to start",
--     cursor = { character = "▌", color = "#39FF88", blink = true, blink_interval_ms = 500 },
--     animation = "cursor_expand",
-- })

-- 3. Theme (importable module with optional local overrides)
egotop.theme({
    theme = matrix_theme.setup,
})

-- 4. Background (independent background canvas layer — does not participate in container layout)
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

-- 6. Flat Independent Containers (egotop calculates positions & sizes automatically)
egotop.layout({
    padding = 1,
    gap = 1,
    min_container_width = 22,
    containers = {
        egotop.container({
            name = "header",
            description = "Top status bar with brand, active mode, effect, and clock",
            region = "top",
            row = 0,
            kind = "header",
            category = "dynamic",
            width = "100%",
            height = 1,
            transparent = true,
        }),
        egotop.container({
            name = "greeting",
            description = "Welcome banner with FIGlet greeting and system logo",
            region = "top",
            row = 1,
            kind = "greeting",
            category = "static",
            width = "100%",
            height = 9,
            min_height = 6,
            align = "center",
            font = "ANSI Shadow",
            border = { enabled = true, title = "Welcome" },
        }),
        egotop.container({
            name = "system",
            description = "Operating system, host, kernel, shell, and terminal metadata",
            region = "main",
            row = 0,
            kind = "system",
            category = "dynamic",
            width = "50%",
            height = "fill",
            min_height = 6,
            padding = 1,
            border = { enabled = true, title = "System" },
        }),
        egotop.container({
            name = "hardware",
            description = "CPU, RAM, disk, and GPU usage telemetry bars",
            region = "main",
            row = 0,
            kind = "hardware",
            category = "dynamic",
            width = "50%",
            height = "fill",
            min_height = 6,
            padding = 1,
            graph_style = "bar",
            border = { enabled = true, title = "Hardware" },
        }),
        egotop.container({
            name = "environment",
            description = "Window manager, session protocol, display, and desktop theme",
            region = "main",
            row = 1,
            kind = "environment",
            category = "dynamic",
            width = "40%",
            height = "fill",
            min_height = 5,
            padding = 1,
            border = { enabled = true, title = "Environment" },
        }),
        egotop.container({
            name = "terminal",
            description = "Terminal emulator, shell, font, and viewport dimensions",
            region = "main",
            row = 1,
            kind = "terminal",
            category = "dynamic",
            width = "30%",
            height = "fill",
            min_height = 5,
            padding = 1,
            border = { enabled = true, title = "Terminal" },
        }),
        egotop.container({
            name = "storage",
            description = "Mounted filesystems and storage capacity bars",
            region = "main",
            row = 1,
            kind = "storage",
            category = "dynamic",
            width = "fill",
            height = "fill",
            min_height = 5,
            padding = 1,
            border = { enabled = true, title = "Storage" },
        }),
        egotop.container({
            name = "activity",
            description = "Load averages, process counts, and CPU/RAM history sparklines",
            region = "main",
            row = 2,
            kind = "activity",
            category = "dynamic",
            width = "40%",
            height = "fill",
            min_height = 6,
            padding = 1,
            graph_style = "sparkline",
            border = { enabled = true, title = "Activity" },
        }),
        egotop.container({
            name = "network",
            description = "Network interfaces and RX/TX transfer telemetry",
            region = "main",
            row = 2,
            kind = "network",
            category = "dynamic",
            width = "30%",
            height = "fill",
            min_height = 6,
            padding = 1,
            border = { enabled = true, title = "Network" },
        }),
        egotop.container({
            name = "session",
            description = "System uptime, boot timestamp, and active session status",
            region = "main",
            row = 2,
            kind = "session",
            category = "dynamic",
            width = "fill",
            height = "fill",
            min_height = 6,
            padding = 1,
            border = { enabled = true, title = "Session & Uptime" },
        }),
        egotop.container({
            name = "status_spinner",
            description = "Footer activity spinner and command-line bar",
            region = "bottom",
            row = 0,
            kind = "spinner",
            category = "animated",
            width = "100%",
            height = 3,
            border = { enabled = true },
            text = "Monitoring system telemetry",
            animation = { fps = 12, speed = 1.0, style = "braille" },
        }),
    },
})
