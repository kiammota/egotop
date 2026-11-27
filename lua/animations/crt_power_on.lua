-- CRT power-on horizontal scan beam reveal animation (isolated canvas API).
local animation = {}

function animation.setup(api)
    local duration = 0.85
    api.ui.reveal(0.0)

    return {
        name = "crt_power_on",
        description = "CRT horizontal scan beam reveal on an isolated canvas",
        fps = 30,
        duration = duration,
        render = function(canvas, ctx)
            local t = math.min(1.0, ctx.t / duration)
            local eased = 1.0 - math.pow(1.0 - t, 4)
            ctx.reveal(eased)
            local w, h = canvas.width, canvas.height
            if w <= 0 or h <= 0 then
                return
            end
            local cy = math.floor(h / 2)
            local span = math.max(1, math.floor(w * eased))
            local start_x = math.floor((w - span) / 2)
            canvas:text(start_x, cy, ("─"):rep(span), "#FFFFFF", nil, 1.0)
        end,
    }
end

return animation
