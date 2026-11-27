-- CRT power-on horizontal scan beam reveal animation (implemented in Lua).
local animation = {}

function animation.setup(api)
    local w = math.max(10, api.screen.width())
    local h = math.max(5, api.screen.height())
    local cy = math.floor(h / 2)
    api.ui.reveal(0.0)

    local line = api.element({
        id = "crt_beam",
        x = 0,
        y = cy,
        text = ("─"):rep(w),
        fg = "#FFFFFF",
        bold = true,
    })

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 850,
        easing = "ease_out_expo",
        on_update = function(p)
            local span = math.max(1, math.floor(w * p))
            local start_x = math.floor((w - span) / 2)
            line:move(start_x, cy)
            line:set_text(("─"):rep(span))
            api.ui.reveal(p)
        end,
        on_complete = function()
            line:destroy()
            api.ui.reveal(1.0)
            api.finish()
        end,
    })
end

return animation
