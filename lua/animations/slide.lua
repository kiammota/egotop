-- Sliding pull-in startup animation (isolated canvas API).
local animation = {}

animation.type = "intro"

function animation.setup(api)
    local duration = 0.8
    api.ui.reveal(0.0)

    return {
        name = "slide",
        description = "Sliding grip reveal inside an isolated canvas",
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
            local x = math.floor(eased * math.max(0, w - 4))
            local y = math.floor(h / 2)
            canvas:text(x, y, ">>>", "#00D9FF", nil, 1.0)
        end,
    }
end

return animation
