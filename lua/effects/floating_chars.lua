-- Drifting floating characters effect (Lua Canvas / Cell-Buffer API).
local floating_chars = {}

floating_chars.type = "background"

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
                phase = math.random() * math.pi * 2.0,
            }
        end
    end

    return {
        name = "floating_chars",
        fps = opts.fps or 24,
        options = opts,
        init = function(ctx)
            init_items(ctx.state, ctx.width, ctx.height)
        end,
        resize = function(ctx)
            init_items(ctx.state, ctx.width, ctx.height)
        end,
        update = function(dt, _t, ctx)
            local state = ctx.state
            if not state.items then
                init_items(state, ctx.width, ctx.height)
            end
            for _, it in ipairs(state.items) do
                it.x = (it.x + it.vx * speed * dt) % math.max(1, ctx.width)
                it.y = (it.y + it.vy * speed * dt) % math.max(1, ctx.height)
            end
        end,
        render = function(canvas, ctx)
            local items = ctx.state.items
            if not items then
                return
            end
            for i, it in ipairs(items) do
                local fg = (i % 2 == 0) and ctx.theme.secondary or ctx.theme.primary
                local intensity = 0.6 + 0.4 * ctx.sin(ctx.t * 2.0 + it.phase)
                canvas:put(it.x, it.y, it.glyph, fg, nil, intensity)
            end
        end,
    }
end

return floating_chars
