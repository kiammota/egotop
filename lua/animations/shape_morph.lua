-- Morphing geometric ASCII emblem animation (isolated canvas API).
local animation = {}

animation.type = "intro"

local FRAMES = {
    { "  /\\  ", " /  \\ ", "/____\\" },
    { " +--+ ", " |  | ", " +--+ " },
    { "  /\\  ", "<    >", "  \\/  " },
    { " .--. ", "| ** |", " '--' " },
}

function animation.setup(api)
    local duration = 1.2
    api.ui.reveal(0.2)

    return {
        name = "shape_morph",
        description = "Morphing geometric ASCII emblem inside an isolated canvas",
        fps = 24,
        duration = duration,
        render = function(canvas, ctx)
            local p = math.min(1.0, ctx.t / duration)
            ctx.reveal(0.2 + p * 0.8)
            local w, h = canvas.width, canvas.height
            if w < 6 or h < 3 then
                return
            end
            local idx = (math.floor(ctx.t / 0.22) % #FRAMES) + 1
            local cx = math.floor(w / 2) - 3
            local cy = math.floor(h / 2) - 1
            canvas:ascii(cx, cy, FRAMES[idx], "#88C0D0", nil, 1.0)
        end,
    }
end

return animation
