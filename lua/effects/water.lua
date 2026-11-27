-- Animated sine wave water surface background effect (implemented in Lua).
local water = {}

local WAVE_CHARS = { " ", "~", "≈", "∿", "〜" }

function water.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = opts.density or 0.65

    return {
        name = "water",
        fps = opts.fps or 24,
        options = opts,
        init = function(state)
            state.phase = 0.0
        end,
        update = function(state, dt)
            state.phase = (state.phase or 0.0) + dt * speed * 2.4
        end,
        render = function(state, draw, ctx)
            local phase = state.phase or 0.0
            local c1 = opts.color or ctx.theme.secondary
            local c2 = ctx.theme.primary
            local step_y = density > 0.7 and 2 or 3

            for y = 0, ctx.height - 1, step_y do
                for x = 0, ctx.width - 1 do
                    local v = math.sin(x * 0.14 + phase + y * 0.45)
                        + 0.5 * math.cos(x * 0.08 - phase * 0.7 + y * 0.25)
                    if v > 0.35 then
                        local ch = WAVE_CHARS[math.min(#WAVE_CHARS, math.max(2, math.floor((v + 1.0) * 1.8)))]
                        local t = math.min(1.0, math.max(0.0, (v + 1.0) * 0.5))
                        draw:cell(x, y, ch, draw:blend(c1, c2, t), nil)
                    end
                end
            end
        end,
    }
end

return water
