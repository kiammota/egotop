-- Cyberpunk theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "cyberpunk",
        background = "#0A0612",
        panel_background = "#100A1C",
        primary = "#00FFD5",
        secondary = "#FF2A85",
        accent = "#FFDD00",
        text = "#EAF2FF",
        muted = "#766896",
        border = "#B4288C",
        danger = "#FF4455",
        border_type = "rounded",
    }, overrides)
end

return theme
