-- Fire & bounce per-character greeting text animation (implemented in Lua).
-- Shows how to animate greeting characters & colors continuously without blocking!
local animation = {}

function animation.setup(api)
    local palette = { "#FF4455", "#FF8800", "#FFDD00", "#FFE8B4" }

    api.every(80, function()
        local t = api.elapsed_ms() / 1000.0
        api.ui.set_greeting_transform(function(ch, col, row)
            if ch == " " then
                return nil
            end
            local wave = math.sin(t * 6.0 + col * 0.35 + row * 0.7)
            local color_idx = math.floor(((wave + 1.0) * 0.5) * (#palette - 1)) + 1
            local dy = wave > 0.75 and -1 or 0
            return {
                char = ch,
                fg = palette[color_idx],
                dy = dy,
                bold = true,
            }
        end)
    end)
end

return animation
