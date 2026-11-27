-- Drifting floating characters background effect (implemented in Lua).
local floating_chars = {}

local function split_utf8(str)
    local out = {}
    for _, code in utf8.codes(str) do
        out[#out + 1] = utf8.char(code)
    end
    return out
end

function floating_chars.setup(opts)
    opts = opts or {}
    local charset = split_utf8(opts.charset or "egotop*+~.")
    local speed = opts.speed or 1.0
    local count = opts.count or 28

    local function init_items(state, w, h)
        state.items = {}
        for i = 1, count do
            state.items[i] = {
                x = math.random() * math.max(1, w),
                y = math.random() * math.max(1, h),
                vx = (math.random() - 0.5) * 3.0,
                vy = (math.random() - 0.5) * 2.0,
                glyph = charset[math.random(1, #charset)],
            }
        end
    end

    return {
        name = "floating_chars",
        fps = opts.fps or 20,
        options = opts,
        init = function(state, ctx)
            init_items(state, ctx.width, ctx.height)
        end,
        resize = function(state, ctx)
            init_items(state, ctx.width, ctx.height)
        end,
        update = function(state, dt, ctx)
            if not state.items then
                init_items(state, ctx.width, ctx.height)
            end
            for _, it in ipairs(state.items) do
                it.x = (it.x + it.vx * speed * dt) % math.max(1, ctx.width)
                it.y = (it.y + it.vy * speed * dt) % math.max(1, ctx.height)
            end
        end,
        render = function(state, draw, ctx)
            if not state.items then
                return
            end
            for i, it in ipairs(state.items) do
                local fg = (i % 2 == 0) and ctx.theme.secondary or ctx.theme.primary
                draw:cell(math.floor(it.x), math.floor(it.y), it.glyph, fg, nil)
            end
        end,
    }
end

return floating_chars
