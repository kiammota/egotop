-- Bamboo theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "bamboo",
        background = "#1B2118",
        panel_background = "#232B1F",
        primary = "#8FB573",
        secondary = "#5FB3B3",
        accent = "#E0B96B",
        text = "#E3E7D8",
        muted = "#7B876F",
        border = "#48593E",
        danger = "#E06C75",
        border_type = "rounded",
    }, overrides)
end

return theme
