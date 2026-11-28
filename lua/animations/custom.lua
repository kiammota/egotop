-- Custom user animation starter template (~/.config/egotop/animations/custom.lua).
-- Uses the isolated Canvas API (`canvas.width`, `canvas.height`, `ctx`).
local animation = {}

animation.type = "intro"

function animation.setup(api)
    local duration = 0.9
    api.ui.reveal(0.0)

    return {
        name = "custom",
        description = "Starter template animation on an isolated canvas",
        fps = 30,
        duration = duration,
        render = function(canvas, ctx)
            local p = math.min(1.0, ctx.t / duration)
            local eased = 1.0 - math.pow(1.0 - p, 3)
            ctx.reveal(eased)
            local w, h = canvas.width, canvas.height
            if w <= 0 or h <= 0 then
                return
            end
            local label = "egotop lua engine"
            local x = math.max(0, math.floor((w - #label) / 2))
            local y = math.floor(h / 2)
            local col = ctx.rgb_lerp("#39FF88", "#00D9FF", eased)
            canvas:text(x, y, label, col, nil, 1.0)
        end,
    }
end

return animation
