-- Festive multi-stage Christmas tree animation built purely in Lua!
-- Demonstrates full creative freedom: sequential tree assembly, twinkling ornaments,
-- falling snow particles, and greeting reveal.
local animation = {}

function animation.setup(api)
    local w = math.max(24, api.screen.width())
    local h = math.max(16, api.screen.height())
    local cx = math.floor(w / 2)
    local top_y = math.max(1, math.floor(h / 2) - 6)

    api.ui.reveal(0.0)

    local tree_rows = {
        "        ★        ",
        "        ▲        ",
        "       ▲▲▲       ",
        "      ▲●▲○▲      ",
        "     ▲▲▲▲▲▲▲     ",
        "    ▲○▲▲★▲▲●▲    ",
        "   ▲▲▲▲▲▲▲▲▲▲▲   ",
        "  ▲●▲▲○▲▲▲●▲▲○▲  ",
        "       ███       ",
    }

    local star = api.element({
        id = "xmas_star",
        x = cx - 8,
        y = top_y,
        text = tree_rows[1],
        fg = "#FFE66D",
        bold = true,
        visible = false,
    })

    local built_rows = {}
    for i = 2, #tree_rows do
        built_rows[i] = api.element({
            id = "xmas_row_" .. i,
            x = cx - 8,
            y = top_y + i - 1,
            text = tree_rows[i],
            fg = (i == #tree_rows) and "#A06400" or "#39FF88",
            bold = true,
            visible = false,
        })
    end

    -- Twinkling snow flakes in parallel
    local flakes = {}
    for i = 1, 14 do
        flakes[i] = {
            x = math.random(0, math.max(0, w - 1)),
            y = math.random(0, math.max(0, h - 1)),
            elem = api.element({
                id = "snow_" .. i,
                x = 0,
                y = 0,
                text = (i % 2 == 0) and "*" or "❄",
                fg = "#E8F1F2",
            }),
        }
    end

    api.every(120, function()
        for _, f in ipairs(flakes) do
            f.y = (f.y + 1) % h
            f.elem:move(f.x, f.y)
        end
        if star then
            local t = (api.elapsed_ms() % 600) / 600.0
            star:set_color(api.lerp_color("#FFE66D", "#FF4FD8", math.abs(t * 2 - 1)), nil)
        end
    end)

    -- Assemble the tree row by row from bottom to top, light the star, then reveal UI!
    local steps = {}
    for i = #tree_rows, 2, -1 do
        steps[#steps + 1] = {
            duration_ms = 140,
            on_start = function()
                built_rows[i]:set_visible(true)
            end,
        }
    end
    steps[#steps + 1] = {
        duration_ms = 500,
        on_start = function()
            star:set_visible(true)
        end,
    }
    steps[#steps + 1] = {
        duration_ms = 900,
        easing = "ease_out_cubic",
        on_update = function(p)
            api.ui.reveal(p)
        end,
    }

    api.sequence(steps, function()
        star:destroy()
        for _, row in pairs(built_rows) do
            row:destroy()
        end
        for _, f in ipairs(flakes) do
            f.elem:destroy()
        end
        api.ui.reveal(1.0)
        api.finish()
    end)
end

return animation
