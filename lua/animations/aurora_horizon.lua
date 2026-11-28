-- Aurora horizon sweep animation (isolated canvas API, adapts automatically on resize).
local animation = {}

animation.type = "intro"

function animation.setup(api)
    local duration = 1.4
    api.ui.reveal(0.0)

    return {
        name = "aurora_horizon",
        description = "Aurora horizon color sweep across an isolated canvas",
        fps = 30,
        duration = duration,
        render = function(canvas, ctx)
            local p = math.min(1.0, ctx.t / duration)
            ctx.reveal(p)
            local w = canvas.width
            if w <= 0 or canvas.height <= 0 then
                return
            end
            local pattern = ("░▒▓█▓▒░"):rep(math.max(1, math.ceil(w / 7)))
            local col = ctx.rgb_lerp("#39FF88", "#00D9FF", p)
            local y = math.min(1, canvas.height - 1)
            canvas:text(0, y, pattern:sub(1, w), col, nil, 1.0)
        end,
    }
end

return animation
