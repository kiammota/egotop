-- Procedural ambient noise field effect (Lua Canvas / Cell-Buffer API).
local noise = {}

noise.type = "background"

local SHADES = { "·", ":", "+", "*" }

function noise.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = math.max(0.05, math.min(1.0, opts.density or 0.35))

    return {
        name = "noise",
        fps = opts.fps or 20,
        options = opts,
        init = function(ctx)
            ctx.state.phase = 0.0
        end,
        update = function(dt, _t, ctx)
            ctx.state.phase = (ctx.state.phase or 0.0) + dt * speed
        end,
        render = function(canvas, ctx)
            local z = (ctx.state.phase or 0.0) * 1.4
            local threshold = 1.0 - density * 0.45
            for y = 0, canvas.height - 1, 2 do
                for x = 0, canvas.width - 1, 2 do
                    local n = ctx.noise(x * 0.22, y * 0.28, z)
                    if n > threshold then
                        local norm = ctx.clamp((n - threshold) / math.max(0.01, 1.0 - threshold), 0.0, 1.0)
                        local idx = math.min(#SHADES, math.max(1, math.floor(norm * #SHADES) + 1))
                        canvas:put(x, y, SHADES[idx], ctx.theme.muted, nil, 0.45 + 0.55 * norm)
                    end
                end
            end
        end,
    }
end

return noise
