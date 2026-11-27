-- Aurora horizon sweep + greeting color shimmer (implemented in Lua).
local animation = {}

function animation.setup(api)
    local w = math.max(20, api.screen.width())
    api.ui.reveal(0.0)

    local banner = api.element({
        id = "aurora_wave",
        x = 0,
        y = 1,
        text = ("░▒▓█▓▒░"):rep(math.max(1, math.floor(w / 7))),
        fg = "#39FF88",
    })

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 1400,
        easing = "ease_out_cubic",
        on_update = function(p)
            banner:set_color(api.lerp_color("#39FF88", "#00D9FF", p), nil)
            api.ui.reveal(p)
        end,
        on_complete = function()
            banner:destroy()
            api.ui.reveal(1.0)
            api.finish()
        end,
    })
end

return animation
