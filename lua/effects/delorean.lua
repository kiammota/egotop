-- Responsive ASCII DMC-12. The sprite is authored in terminal-cell aspect ratio
-- (about 5:1 cells, which reads as a 2.5:1 physical silhouette).
local delorean = { type = "canvas" }

local art = {
    "                    _________                         ",
    "              _____/  _____  \\____                   ",
    "         ____/  ___/__/     \\__\\___\\________         ",
    "    ____/  _/__/  ______________  \\_   ___  \\___     ",
    " __/  _/__/  _/__/  _  _  _  _  \\__\\_/   \\___  \\___",
    "|  _/___/___/__/__/|_|_|_|_|_|_|\\__\\___     _\\___  ",
    "|_/  @   *  ___   |                  |   ___\\__/  \\__|",
    "  \\__=====/___\\__|______________|__/___\\====__/     ",
    "      O----O                               O----O       ",
}
local compact_art = {
    "       _____________          ",
    "  ____/  _  _  _  _  \\________",
    " /___/|              |\\____    ",
    "|_@__|______________|___*_|   ",
    "   O----O        O----O        ",
}

local function color_for(ch, pulse)
    if ch == "@" or ch == "*" then
        return pulse > 0.45 and "#FFB347" or "#FF6B35"
    elseif ch == "=" or ch == ":" then
        return "#FF4FD8"
    elseif ch == "D" or ch == "M" or ch == "C" or ch == "1" or ch == "2" then
        return "#35D8FF"
    elseif ch == "|" or ch == "_" or ch == "/" or ch == "\\" or ch == "-" then
        return "#C8D3DC"
    elseif ch == "O" then
        return "#E9F4FF"
    end
    return "#7FA6B8"
end

function delorean.setup(opts)
    opts = opts or {}
    return {
        name = "delorean",
        fps = opts.fps or 24,
        init = function(ctx)
            ctx.state.phase = 0
            ctx.state.bob = 0
        end,
        on_resize = function(width, height, ctx)
            -- Geometry is derived at render time; retain phase and motion state.
            ctx.state.last_width, ctx.state.last_height = width, height
        end,
        update = function(dt, t, ctx)
            ctx.state.phase = (ctx.state.phase or 0) + math.min(dt, 0.05)
            ctx.state.bob = ctx.sin(t * 5.4) * 0.45
        end,
        render = function(canvas, ctx)
            local width, height = canvas.width or 0, canvas.height or 0
            if width < 4 or height < 3 then return end
            local pulse = 0.5 + 0.5 * ctx.sin(ctx.t * 5.0)
            local sprite = (width < 60 or height < 18) and compact_art or art
            local sprite_h, sprite_w = #sprite, 0
            for _, line in ipairs(sprite) do sprite_w = math.max(sprite_w, #line) end
            local scale = math.min(width / sprite_w, (height - 1) / sprite_h)
            if scale <= 0 then return end
            local draw_w = math.max(1, math.floor(sprite_w * scale))
            local draw_h = math.max(1, math.floor(sprite_h * scale))
            local x0 = math.floor((width - draw_w) / 2)
            local y0 = math.floor((height - draw_h) / 2 + (ctx.state.bob or 0))

            -- Speed streaks slide behind the car, remaining clear of the sprite.
            local trail_count = math.max(2, math.floor(2 + 7 * (ctx.lod or 0.5)))
            for i = 1, trail_count do
                local row = (i * 7 + math.floor(ctx.t * 9)) % math.max(1, draw_h - 2)
                local len = math.max(2, math.floor(draw_w * (0.10 + (i % 4) * 0.035)))
                local end_x = math.max(0, x0 - 1 - ((i * 3 + math.floor(ctx.t * 12)) % math.max(1, math.floor(draw_w * 0.12))))
                local start_x = math.max(0, end_x - len)
                for x = start_x, end_x do
                    canvas:put(x, y0 + row, i % 3 == 0 and "-" or ".", i % 3 == 0 and "#35D8FF" or "#315A70")
                end
            end

            for dy = 0, draw_h - 1 do
                local sy = math.min(sprite_h, math.floor(dy / scale) + 1)
                local line = sprite[sy] or ""
                for dx = 0, draw_w - 1 do
                    local sx = math.min(sprite_w, math.floor(dx / scale) + 1)
                    local ch = line:sub(sx, sx)
                    if ch ~= " " then
                        local y = y0 + dy
                        if y >= 0 and y < height then
                            canvas:put(x0 + dx, y, ch, color_for(ch, pulse))
                        end
                    end
                end
            end
            canvas:text(
                x0 + math.floor(draw_w * 0.5) - 3,
                y0 + math.floor(draw_h * 0.58),
                "DMC-12",
                "#35D8FF"
            )
            -- Amber exhaust/flux accents flicker beneath the rear quarter.
            if (ctx.lod or 0) > 0.2 then
                local exhaust_x = math.max(0, x0 + math.floor(draw_w * 0.10))
                local exhaust_y = math.min(height - 1, y0 + draw_h - 2)
                canvas:put(exhaust_x, exhaust_y, pulse > 0.5 and "*" or ":", "#FF9F1C")
                if (ctx.lod or 0) > 0.55 and exhaust_x > 1 then
                    canvas:put(exhaust_x - 1, exhaust_y, ".", "#FF4FD8")
                end
            end
        end,
    }
end

return delorean
