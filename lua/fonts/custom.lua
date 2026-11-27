-- Example custom ASCII font module for EgoTop.
-- Edit or duplicate this file to build your own fonts under ~/.config/egotop/fonts/.
local bitmap = require("fonts._bitmap")
local font = {}

function font.setup()
    -- Start with full coverage from 5x5 glyphs, then override any character explicitly:
    local characters = bitmap.map_glyphs(function(bmp)
        local lines = {}
        for _, row in ipairs(bmp) do
            lines[#lines + 1] = row:gsub("#", "██"):gsub(" ", "  ") .. " "
        end
        return lines
    end)

    characters["A"] = {
        "  ████  ",
        " ██  ██ ",
        "████████",
        "██    ██",
        "██    ██",
    }

    return {
        name = "Custom",
        height = 5,
        spacing = 1,
        fallback = "?",
        characters = characters,
    }
end

return font
