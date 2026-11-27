-- Glass Neon transparent theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "glass-neon",
        background = "transparent",
        panel_background = "transparent",
        primary = "#39FF88",
        secondary = "#FF4FD8",
        accent = "#FFE66D",
        text = "#E8F1F2",
        muted = "#7D8CA3",
        border = "#28B872",
        danger = "#FF5577",
        border_type = "rounded",
    }, overrides)
end

return theme
