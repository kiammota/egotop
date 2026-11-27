-- Procedural ambient noise field background effect (implemented in Lua).
local noise = {}

local SHADES = { "·", ":", "+", "*" }

function noise.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = math.max(0.05, math.min(1.0, opts.density or 0.35))

    return {
        name = "noise",
        fps = opts.fps or 15,
        options = opts,
        init = function(state)
            state.t = 0.0
        end,
        update = function(state, dt)
            state.t = (state.t or 0.0) + dt * speed
        end,
        render = function(state, draw, ctx)
            local step_t = math.floor((state.t or 0.0) * 6.0)
            local threshold = 1.0 - density * 0.4
            for y = 0, ctx.height - 1, 2 do
                for x = 0, ctx.width - 1, 2 do
                    local h = math.abs(math.sin(x * 12.9898 + y * 78.233 + step_t * 1.37) * 43758.5453)
                    local frac = h - math.floor(h)
                    if frac > threshold then
                        local idx = math.min(#SHADES, math.max(1, math.floor(frac * #SHADES) + 1))
                        draw:cell(x, y, SHADES[idx], ctx.theme.muted, nil)
                    end
                end
            end
        end,
    }
end

return noise
