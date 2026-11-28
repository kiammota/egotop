-- CRT horizontal scanlines effect (Lua Canvas / Cell-Buffer API).
local scanlines = {}

scanlines.type = "background"

function scanlines.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local spacing = math.max(1, opts.spacing or 2)
    local char = opts.char or "─"

    return {
        name = "scanlines",
        fps = opts.fps or 24,
        options = opts,
        init = function(ctx)
            ctx.state.offset = 0.0
        end,
        update = function(dt, _t, ctx)
            ctx.state.offset = (ctx.state.offset or 0.0) + dt * speed * 6.0
        end,
        render = function(canvas, ctx)
            local beam_y = math.floor(ctx.state.offset or 0) % math.max(1, canvas.height)
            local line_color = opts.color or ctx.theme.muted
            local beam_color = ctx.theme.primary
            for y = 0, canvas.height - 1 do
                if y == beam_y then
                    for x = 0, canvas.width - 1 do
                        canvas:put(x, y, "═", beam_color, nil, 1.0)
                    end
                elseif y % spacing == 0 then
                    for x = 0, canvas.width - 1 do
                        canvas:put(x, y, char, line_color, nil, 0.55)
                    end
                end
            end
        end,
    }
end

return scanlines
