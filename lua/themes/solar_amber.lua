-- Solar Amber theme module (solid bg + transparent panels).
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "solar-amber",
        background = "#120C06",
        panel_background = "transparent",
        primary = "#FFB000",
        secondary = "#FFD65A",
        accent = "#FF6E28",
        text = "#FFE8B4",
        muted = "#8C6020",
        border = "#A06400",
        danger = "#FF4632",
        border_type = "plain",
    }, overrides)
end

return theme
