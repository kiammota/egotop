-- Morphing geometric ASCII emblem animation (implemented in Lua).
local animation = {}

function animation.setup(api)
    local w = math.max(16, api.screen.width())
    local h = math.max(10, api.screen.height())
    local cx = math.floor(w / 2) - 3
    local cy = math.floor(h / 2) - 1
    api.ui.reveal(0.2)

    local frames = {
        { "  /\\  ", " /  \\ ", "/____\\" },
        { " +--+ ", " |  | ", " +--+ " },
        { "  /\\  ", "<    >", "  \\/  " },
        { " .--. ", "| ** |", " '--' " },
    }

    local shape = api.element({
        id = "morph_shape",
        x = cx,
        y = cy,
        lines = frames[1],
        fg = "#88C0D0",
        bold = true,
    })

    local idx = 1
    api.every(220, function()
        idx = (idx % #frames) + 1
        shape:set_lines(frames[idx])
    end)

    api.after(1200, function()
        shape:destroy()
        api.ui.reveal(1.0)
        api.finish()
    end)
end

return animation
