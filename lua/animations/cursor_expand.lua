-- Cursor expansion ripple reveal animation (isolated canvas API).
local animation = {}

animation.type = "intro"

function animation.setup(api)
    local duration = 0.75
    api.ui.reveal(0.0)

    return {
        name = "cursor_expand",
        description = "Cursor expansion ripple inside its isolated canvas",
        fps = 30,
        duration = duration,
        render = function(canvas, ctx)
            local t = math.min(1.0, ctx.t / duration)
            local eased = 1.0 - math.pow(1.0 - t, 3)
            ctx.reveal(eased)
            local w, h = canvas.width, canvas.height
            if w <= 0 or h <= 0 then
                return
            end
            local cx = math.floor(w / 2)
            local cy = math.floor(h / 2)
            local span = math.max(1, math.floor(1 + eased * math.min(w, 28)))
            local x0 = math.max(0, cx - math.floor(span / 2))
            canvas:text(x0, cy, ("═"):rep(span), "#39FF88", nil, 1.0)
        end,
    }
end

return animation
