-- Smooth reveal fade animation (implemented in Lua).
local animation = {}

animation.type = "intro"

function animation.setup(api)
    api.ui.reveal(0.0)
    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 650,
        easing = "ease_out_quad",
        on_update = function(p)
            api.ui.reveal(p)
        end,
        on_complete = function()
            api.ui.reveal(1.0)
            api.finish()
        end,
    })
end

return animation
