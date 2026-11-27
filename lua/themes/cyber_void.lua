-- Cyber Void theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "cyber-void",
        background = "#090612",
        panel_background = "#140E24",
        primary = "#00FFD5",
        secondary = "#FF2A85",
        accent = "#FFDD00",
        text = "#EAF2FF",
        muted = "#766896",
        border = "#B4288C",
        danger = "#FF4455",
        border_type = "double",
    }, overrides)
end

return theme
