-- Theater curtains parted reveal animation (implemented in Lua).
local animation = {}

function animation.setup(api)
    api.ui.reveal(0.0)
    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 800,
        easing = "ease_in_out_cubic",
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
