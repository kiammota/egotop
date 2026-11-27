-- Built-in Small compact font module (written in Lua).
local bitmap = require("fonts._bitmap")
local font = {}

function font.setup()
    local characters = bitmap.map_glyphs(function(bmp)
        local rows = {}
        for r = 1, #bmp, 2 do
            local upper = bmp[r]
            local lower = bmp[r + 1] or (" "):rep(#upper)
            local out = {}
            for c = 1, #upper do
                local u = upper:sub(c, c) == "#"
                local l = lower:sub(c, c) == "#"
                if u and l then
                    out[#out + 1] = "█"
                elseif u then
                    out[#out + 1] = "▀"
                elseif l then
                    out[#out + 1] = "▄"
                else
                    out[#out + 1] = " "
                end
            end
            rows[#rows + 1] = table.concat(out) .. " "
        end
        return rows
    end)

    return {
        name = "Small",
        height = 3,
        spacing = 1,
        fallback = " ",
        characters = characters,
    }
end

return font
