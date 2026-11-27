-- Multi-depth starfield background effect (implemented in Lua).
local starfield = {}

local STARS = { ".", "·", "*", "✦" }

local function seed(state, w, h, density)
    state.stars = {}
    local count = math.max(12, math.floor((w * h * density) / 40))
    for i = 1, count do
        local depth = math.random(1, 4)
        state.stars[i] = {
            x = math.random() * math.max(1, w),
            y = math.random() * math.max(1, h),
            depth = depth,
            glyph = STARS[depth],
        }
    end
end

function starfield.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = math.max(0.05, math.min(1.0, opts.density or 0.5))

    return {
        name = "starfield",
        fps = opts.fps or 24,
        options = opts,
        init = function(state, ctx)
            seed(state, ctx.width, ctx.height, density)
        end,
        resize = function(state, ctx)
            seed(state, ctx.width, ctx.height, density)
        end,
        update = function(state, dt, ctx)
            if not state.stars then
                seed(state, ctx.width, ctx.height, density)
            end
            for _, s in ipairs(state.stars) do
                s.x = s.x - s.depth * 2.2 * speed * dt
                if s.x < 0 then
                    s.x = math.max(0, ctx.width - 1)
                    s.y = math.random() * math.max(1, ctx.height)
                end
            end
        end,
        render = function(state, draw, ctx)
            if not state.stars then
                return
            end
            for _, s in ipairs(state.stars) do
                local fg = draw:blend(ctx.theme.muted, ctx.theme.text, s.depth / 4.0)
                draw:cell(math.floor(s.x), math.floor(s.y), s.glyph, fg, nil)
            end
        end,
    }
end

return starfield
