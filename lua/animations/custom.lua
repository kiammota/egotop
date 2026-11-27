-- Custom user animation starter template (~/.config/egotop/animations/custom.lua).
-- Every animation module exposes a single entry point: `setup(api)`.
local animation = {}

function animation.setup(api)
    local w = math.max(20, api.screen.width())
    local h = math.max(8, api.screen.height())

    api.ui.reveal(0.0)
    local label = api.element({
        id = "custom_intro",
        x = math.max(0, math.floor((w - 18) / 2)),
        y = math.floor(h / 2),
        text = "egotop lua engine",
        fg = "#39FF88",
        bold = true,
    })

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 900,
        easing = "ease_out_cubic",
        on_update = function(p)
            label:set_color(api.lerp_color("#39FF88", "#00D9FF", p), nil)
            api.ui.reveal(p)
        end,
        on_complete = function()
            label:destroy()
            api.ui.reveal(1.0)
            api.finish()
        end,
    })
end

return animation
