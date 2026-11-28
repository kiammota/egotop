-- Animated sine wave water surface effect (Lua Canvas / Cell-Buffer API).
local water = {}

water.type = "background"

local WAVE_CHARS = { " ", "~", "≈", "∿", "〜" }

function water.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = opts.density or 0.65

    return {
        name = "water",
        fps = opts.fps or 30,
        options = opts,
        init = function(ctx)
            ctx.state.phase = 0.0
        end,
        update = function(dt, _t, ctx)
            ctx.state.phase = (ctx.state.phase or 0.0) + dt * speed * 2.4
        end,
        render = function(canvas, ctx)
            local phase = ctx.state.phase or 0.0
            local c1 = opts.color or ctx.theme.secondary
            local c2 = ctx.theme.primary
            local step_y = density > 0.7 and 2 or 3

            for y = 0, canvas.height - 1, step_y do
                for x = 0, canvas.width - 1 do
                    local v = ctx.sin(x * 0.14 + phase + y * 0.45)
                        + 0.5 * ctx.cos(x * 0.08 - phase * 0.7 + y * 0.25)
                    if v > 0.35 then
                        local ch = WAVE_CHARS[math.min(#WAVE_CHARS, math.max(2, math.floor((v + 1.0) * 1.8)))]
                        local blend_t = ctx.clamp((v + 1.0) * 0.5, 0.0, 1.0)
                        local fg = canvas:blend(c1, c2, blend_t)
                        canvas:put(x, y, ch, fg, nil, 0.6 + 0.4 * blend_t)
                    end
                end
            end
        end,
    }
end

return water
