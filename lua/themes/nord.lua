-- Nord theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "nord",
        background = "#2E3440",
        panel_background = "#3B4252",
        primary = "#88C0D0",
        secondary = "#A3BE8C",
        accent = "#EBCB8B",
        text = "#ECEFF4",
        muted = "#81A1C1",
        border = "#4C566A",
        danger = "#BF616A",
        border_type = "rounded",
    }, overrides)
end

return theme
