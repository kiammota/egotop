-- Obsidian Solid theme module for EgoTop.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "obsidian-solid",
        background = "#0D0F14",
        panel_background = "#161922",
        primary = "#61AFEF",
        secondary = "#56B6C2",
        accent = "#E5C07B",
        text = "#ABB2BF",
        muted = "#5C6370",
        border = "#2C323C",
        danger = "#E06C75",
        border_type = "rounded",
    }, overrides)
end

return theme
