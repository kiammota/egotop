-- Cyber boot sequence: scans a neon laser bar vertically while revealing the UI.
local animation = {}

function animation.setup(api)
    local w = math.max(10, api.screen.width())
    local h = math.max(5, api.screen.height())
    api.ui.reveal(0.0)

    local beam = api.element({
        id = "cyber_beam",
        x = 0,
        y = 0,
        text = ("━"):rep(w),
        fg = "#00FFD5",
        bold = true,
    })

    local badge = api.element({
        id = "cyber_badge",
        x = math.max(0, math.floor((w - 20) / 2)),
        y = math.max(0, math.floor(h / 2)),
        text = "[ cyberlink online ]",
        fg = "#FF2A85",
        bold = true,
    })

    api.sequence({
        {
            duration_ms = 900,
            easing = "ease_in_out_cubic",
            on_update = function(p)
                local y = math.floor(p * math.max(0, h - 1))
                beam:move(0, y)
                api.ui.reveal(p * 0.75)
            end,
        },
        {
            duration_ms = 350,
            easing = "ease_out_quad",
            on_update = function(p)
                api.ui.reveal(0.75 + p * 0.25)
                badge:set_color(api.lerp_color("#FF2A85", "#00FFD5", p), nil)
            end,
        },
    }, function()
        beam:destroy()
        badge:destroy()
        api.ui.reveal(1.0)
        api.finish()
    end)
end

return animation
