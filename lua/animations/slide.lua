-- Sliding hand/panel pull-in startup animation (implemented in Lua).
local animation = {}

function animation.setup(api)
    local h = math.max(5, api.screen.height())
    local w = math.max(10, api.screen.width())
    api.ui.reveal(0.0)

    local puller = api.element({
        id = "slide_grip",
        x = 0,
        y = math.floor(h / 2),
        text = ">>>",
        fg = "#00D9FF",
        bold = true,
    })

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 800,
        easing = "ease_out_cubic",
        on_update = function(p)
            puller:move(math.floor(p * math.max(0, w - 4)), math.floor(h / 2))
            api.ui.reveal(p)
        end,
        on_complete = function()
            puller:destroy()
            api.ui.reveal(1.0)
            api.finish()
        end,
    })
end

return animation
