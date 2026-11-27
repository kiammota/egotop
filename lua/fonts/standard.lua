-- Built-in Standard outline font module (written in Lua).
local bitmap = require("fonts._bitmap")
local font = {}

local function lit(grid, r, c)
    local row = grid[r]
    return row ~= nil and row[c] == true
end

function font.setup()
    local characters = bitmap.map_glyphs(function(bmp)
        local h = #bmp
        local w = #bmp[1]
        local grid = {}
        for r = 1, h do
            grid[r] = {}
            for c = 1, w do
                grid[r][c] = bmp[r]:sub(c, c) == "#"
            end
        end

        local rows = {}
        for r = 1, h do
            local out = {}
            for c = 1, w do
                if lit(grid, r, c) then
                    local up = lit(grid, r - 1, c)
                    local down = lit(grid, r + 1, c)
                    local left = lit(grid, r, c - 1)
                    local right = lit(grid, r, c + 1)
                    if not up and down and not left and right then
                        out[#out + 1] = "/"
                    elseif not up and down and left and not right then
                        out[#out + 1] = "\\"
                    elseif up and not down and not left and right then
                        out[#out + 1] = "\\"
                    elseif up and not down and left and not right then
                        out[#out + 1] = "/"
                    elseif up and down then
                        out[#out + 1] = "|"
                    elseif not up and not down then
                        out[#out + 1] = "_"
                    else
                        out[#out + 1] = "#"
                    end
                else
                    out[#out + 1] = " "
                end
            end
            out[#out + 1] = " "
            rows[r] = table.concat(out)
        end
        return rows
    end)

    return {
        name = "Standard",
        height = 5,
        spacing = 1,
        fallback = "?",
        characters = characters,
    }
end

return font
