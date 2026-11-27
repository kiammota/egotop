-- Frosted Slate theme module (transparent terminal bg + solid panels).
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "frosted-slate",
        background = "transparent",
        panel_background = "#1E2430",
        primary = "#88C0D0",
        secondary = "#A3BE8C",
        accent = "#EBCB8B",
        text = "#ECEFF4",
        muted = "#7B88A1",
        border = "#4C566A",
        danger = "#BF616A",
        border_type = "rounded",
    }, overrides)
end

return theme
