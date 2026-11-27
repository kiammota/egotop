-- Built-in ANSI Shadow font module (written in Lua).
local bitmap = require("fonts._bitmap")
local font = {}

function font.setup()
    local characters = bitmap.map_glyphs(function(bmp)
        local rows = {}
        for r, row in ipairs(bmp) do
            local out = {}
            for c = 1, #row do
                local ch = row:sub(c, c)
                if ch == "#" then
                    out[#out + 1] = "██"
                else
                    local prev = c > 1 and row:sub(c - 1, c - 1) == "#"
                    out[#out + 1] = prev and "╗ " or "  "
                end
            end
            rows[r] = table.concat(out)
        end
        rows[#rows + 1] = (" "):rep(#rows[1])
        return rows
    end)

    return {
        name = "ANSI Shadow",
        height = 6,
        spacing = 1,
        fallback = " ",
        characters = characters,
    }
end

return font
