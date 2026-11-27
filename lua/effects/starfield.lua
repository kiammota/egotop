-- Multi-depth parallax starfield effect (Lua Canvas / Cell-Buffer API).
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
            twinkle = math.random() * math.pi * 2.0,
        }
    end
end

function starfield.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = math.max(0.05, math.min(1.0, opts.density or 0.5))

    return {
        name = "starfield",
        fps = opts.fps or 30,
        options = opts,
        init = function(ctx)
            seed(ctx.state, ctx.width, ctx.height, density)
        end,
        resize = function(ctx)
            seed(ctx.state, ctx.width, ctx.height, density)
        end,
        update = function(dt, _t, ctx)
            local state = ctx.state
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
        render = function(canvas, ctx)
            local stars = ctx.state.stars
            if not stars then
                return
            end
            for _, s in ipairs(stars) do
                local depth_t = s.depth / 4.0
                local fg = canvas:blend(ctx.theme.muted, ctx.theme.text, depth_t)
                local intensity = 0.5 + 0.5 * ctx.sin(ctx.t * 2.5 + s.twinkle)
                canvas:put(s.x, s.y, s.glyph, fg, nil, 0.35 + 0.65 * depth_t * intensity)
            end
        end,
    }
end

return starfield
