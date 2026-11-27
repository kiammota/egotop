-- Matrix theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "matrix",
        background = "#050A07",
        panel_background = "#08110C",
        primary = "#39FF88",
        secondary = "#00D9FF",
        accent = "#FFC857",
        text = "#E5E5E5",
        muted = "#5F7D6B",
        border = "#1F7A4C",
        danger = "#FF5555",
        border_type = "rounded",
    }, overrides)
end

return theme
