-- Built-in Slant ASCII font module (written in Lua).
local standard = require("fonts.standard")
local font = {}

function font.setup()
    local base = standard.setup()
    local characters = {}
    for ch, lines in pairs(base.characters) do
        local h = #lines
        local slanted = {}
        for r, line in ipairs(lines) do
            local pad = (" "):rep(math.max(0, h - r))
            local trail = (" "):rep(math.max(0, r - 1))
            slanted[r] = pad .. line .. trail
        end
        characters[ch] = slanted
    end

    return {
        name = "Slant",
        height = base.height,
        spacing = 0,
        fallback = "?",
        characters = characters,
    }
end

return font
