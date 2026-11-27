-- Template custom theme module for EgoTop.
-- Edit ~/.config/egotop/themes/custom.lua to build your own palette.
local util = require("themes._util")
local theme = {}

function theme.setup(overrides)
    return util.merge({
        name = "custom",
        background = "#101418",
        panel_background = "#181E24",
        primary = "#56D364",
        secondary = "#58A6FF",
        accent = "#D29922",
        text = "#E6EDF3",
        muted = "#7D8590",
        border = "#30363D",
        danger = "#F85149",
        border_type = "rounded",
    }, overrides)
end

return theme
