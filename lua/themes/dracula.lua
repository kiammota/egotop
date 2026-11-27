-- Dracula theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "dracula",
        background = "#1E1F29",
        panel_background = "#282A36",
        primary = "#50FA7B",
        secondary = "#8BE9FD",
        accent = "#F1FA8C",
        text = "#F8F8F2",
        muted = "#6272A4",
        border = "#BD93F9",
        danger = "#FF5555",
        border_type = "rounded",
    }, overrides)
end

return theme
