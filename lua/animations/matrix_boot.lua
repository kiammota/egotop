-- Matrix boot startup animation (isolated canvas API, adapts cleanly to resize).
local animation = {}

local GLYPHS = { "0", "1", "ﾊ", "ﾐ", "ｼ", "ｱ", "ﾎ", "ﾈ", "ｽ" }

local function seed_streams(state, w, h)
    state.streams = {}
    state.w = w
    state.h = h
    for x = 0, math.max(0, w - 1), 3 do
        state.streams[#state.streams + 1] = {
            x = x,
            y = -math.random(0, math.max(1, math.floor(h / 2))),
            speed = 18.0 + math.random() * 14.0,
        }
    end
end

function animation.setup(api)
    local duration = 1.2
    api.ui.reveal(0.0)

    return {
        name = "matrix_boot",
        description = "Matrix glyph stream startup reveal on an isolated canvas",
        fps = 30,
        duration = duration,
        init = function(ctx)
            seed_streams(ctx.state, ctx.width, ctx.height)
        end,
        resize = function(ctx)
            seed_streams(ctx.state, ctx.width, ctx.height)
        end,
        update = function(dt, t, ctx)
            if not ctx.state.streams then
                seed_streams(ctx.state, ctx.width, ctx.height)
            end
            local h = math.max(1, ctx.height)
            for _, s in ipairs(ctx.state.streams) do
                s.y = (s.y + s.speed * dt) % h
            end
            local p = math.min(1.0, t / duration)
            ctx.reveal(1.0 - math.pow(1.0 - p, 3))
        end,
        render = function(canvas, ctx)
            local streams = ctx.state.streams
            if not streams then
                return
            end
            for _, s in ipairs(streams) do
                local iy = math.floor(s.y)
                local g = GLYPHS[(iy % #GLYPHS) + 1]
                canvas:put(s.x, iy, g, "#39FF88", nil, 1.0)
                if iy - 1 >= 0 then
                    local g2 = GLYPHS[((iy - 1) % #GLYPHS) + 1]
                    canvas:put(s.x, iy - 1, g2, "#1F7A4C", nil, 0.7)
                end
            end
        end,
    }
end

return animation
