-- CRT horizontal scanlines background effect (implemented in Lua).
local scanlines = {}

function scanlines.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local spacing = math.max(1, opts.spacing or 2)
    local char = opts.char or "─"

    return {
        name = "scanlines",
        fps = opts.fps or 20,
        options = opts,
        init = function(state)
            state.offset = 0.0
        end,
        update = function(state, dt)
            state.offset = (state.offset or 0.0) + dt * speed * 6.0
        end,
        render = function(state, draw, ctx)
            local beam_y = math.floor(state.offset or 0) % math.max(1, ctx.height)
            local line_color = opts.color or ctx.theme.muted
            local beam_color = ctx.theme.primary
            for y = 0, ctx.height - 1 do
                if y == beam_y then
                    draw:fill(0, y, ctx.width, 1, "═", beam_color, nil)
                elseif y % spacing == 0 then
                    draw:fill(0, y, ctx.width, 1, char, line_color, nil)
                end
            end
        end,
    }
end

return scanlines
