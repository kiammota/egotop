-- Procedural ASCII flame showcase inside its own isolated boxed canvas.
-- Can also be mounted directly inside any container via `script = "animations.greeting_fire"`.
local animation = {}

animation.type = "intro"

local FLAME_CHARS = { " ", ".", ":", "*", "+", "#", "%", "@" }
local PALETTE = { "#551100", "#AA2200", "#FF4422", "#FF8800", "#FFDD00", "#FFF3B0" }

function animation.setup(api)
    if api and api.ui and api.ui.reveal then
        api.ui.reveal(1.0)
    end

    return {
        name = "greeting_fire",
        description = "Procedural ASCII flame effect inside an isolated canvas",
        layer = "boxed",
        title = "✦ Greeting Fire ✦",
        fps = 24,
        duration = 4.0,
        render = function(canvas, ctx)
            local w, h = canvas.width, canvas.height
            if w < 4 or h < 3 then
                return
            end
            local t = ctx.t
            for y = 0, h - 1 do
                local ny = (h - 1 - y) / math.max(1, h - 1)
                for x = 0, w - 1 do
                    local nx = x * 0.22
                    local n = ctx.noise(nx, y * 0.35 + t * 4.5, t * 1.3)
                    local heat = ctx.clamp((1.0 - ny * 0.95) * (0.45 + n * 0.85), 0.0, 1.0)
                    if heat > 0.16 then
                        local ch_idx = math.min(#FLAME_CHARS, math.max(2, math.floor(heat * #FLAME_CHARS) + 1))
                        local pal_idx = math.min(#PALETTE, math.max(1, math.floor(heat * #PALETTE) + 1))
                        canvas:put(x, y, FLAME_CHARS[ch_idx], PALETTE[pal_idx], nil, 0.5 + 0.6 * heat)
                    end
                end
            end
            local title = "EGOTOP"
            local tx = math.max(0, math.floor((w - #title) / 2))
            local ty = math.floor(h / 2)
            canvas:text(tx, ty, title, "#FFF6B3", nil, 1.2)
        end,
    }
end

return animation
