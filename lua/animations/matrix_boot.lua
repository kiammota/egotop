-- Matrix boot startup animation (implemented purely via EgoTop Lua Animation API).
-- Gradually reveals the dashboard while streaming matrix glyphs across unlocked areas.
local animation = {}

function animation.setup(api)
    local duration = 1200
    api.ui.reveal(0.0)

    local glyphs = { "0", "1", "ﾊ", "ﾐ", "ｼ", "ｱ", "ﾎ", "ﾈ", "ｽ" }
    local streams = {}
    local w = math.max(20, api.screen.width())
    local h = math.max(10, api.screen.height())

    for x = 1, w, 3 do
        local elem = api.element({
            id = "matrix_col_" .. x,
            x = x - 1,
            y = -math.random(0, 6),
            text = glyphs[math.random(1, #glyphs)],
            fg = "#39FF88",
            bold = true,
        })
        streams[#streams + 1] = { elem = elem, x = x - 1, y = -math.random(0, 5), speed = 18 + math.random() * 14 }
    end

    api.tween({
        from = 0.0,
        to = 1.0,
        duration_ms = duration,
        easing = "ease_out_cubic",
        on_update = function(p)
            api.ui.reveal(p)
            local dt = api.dt()
            for _, s in ipairs(streams) do
                s.y = s.y + s.speed * dt
                if s.y >= h then
                    s.y = 0
                end
                local g = glyphs[(math.floor(s.y) % #glyphs) + 1]
                s.elem:move(s.x, math.floor(s.y))
                s.elem:set_text(g)
            end
        end,
        on_complete = function()
            api.ui.reveal(1.0)
            for _, s in ipairs(streams) do
                s.elem:destroy()
            end
            api.finish()
        end,
    })
end

return animation
