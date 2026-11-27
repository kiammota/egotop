-- Cursor expansion ripple reveal animation (implemented in Lua).
local animation = {}

function animation.setup(api)
    local w = math.max(10, api.screen.width())
    local h = math.max(6, api.screen.height())
    local cx = math.floor(w / 2)
    local cy = math.floor(h / 2)
    api.ui.reveal(0.0)

    local cursor = api.element({
        id = "cursor_expand_glyph",
        x = cx,
        y = cy,
        text = "▌",
        fg = "#39FF88",
        bold = true,
    })

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = 750,
        easing = "ease_out_cubic",
        on_update = function(p)
            local span = math.max(1, math.floor(1 + p * math.min(w, 28)))
            cursor:move(math.max(0, cx - math.floor(span / 2)), cy)
            cursor:set_text(("═"):rep(span))
            api.ui.reveal(p)
        end,
        on_complete = function()
            cursor:destroy()
            api.ui.reveal(1.0)
            api.finish()
        end,
    })
end

return animation
