-- Built-in Banner '#' font module (written in Lua).
local bitmap = require("fonts._bitmap")
local font = {}

function font.setup()
    local characters = bitmap.map_glyphs(function(bmp)
        local rows = {}
        for r, row in ipairs(bmp) do
            rows[r] = row:gsub("#", "##") .. " "
        end
        return rows
    end)

    return {
        name = "Banner",
        height = 5,
        spacing = 1,
        fallback = " ",
        characters = characters,
    }
end

return font
