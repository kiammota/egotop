-- Floating glowing particles background effect (implemented in Lua).
local particles = {}

local GLYPHS = { "·", "•", "◦", "✦", "✧", "*" }

local function seed_particles(state, w, h, density)
    state.items = {}
    local count = math.max(8, math.floor((w * h * density) / 48))
    for i = 1, count do
        state.items[i] = {
            x = math.random() * math.max(1, w),
            y = math.random() * math.max(1, h),
            vx = (math.random() - 0.5) * 4.5,
            vy = -(0.8 + math.random() * 2.8),
            glyph = GLYPHS[math.random(1, #GLYPHS)],
            hue = math.random(),
        }
    end
end

function particles.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local density = math.max(0.05, math.min(1.0, opts.density or 0.55))

    return {
        name = "particles",
        fps = opts.fps or 24,
        options = opts,
        init = function(state, ctx)
            seed_particles(state, ctx.width, ctx.height, density)
        end,
        resize = function(state, ctx)
            seed_particles(state, ctx.width, ctx.height, density)
        end,
        update = function(state, dt, ctx)
            if not state.items then
                seed_particles(state, ctx.width, ctx.height, density)
            end
            for _, p in ipairs(state.items) do
                p.x = (p.x + p.vx * speed * dt) % math.max(1, ctx.width)
                p.y = p.y + p.vy * speed * dt
                if p.y < 0 then
                    p.y = math.max(0, ctx.height - 1)
                    p.x = math.random() * math.max(1, ctx.width)
                end
            end
        end,
        render = function(state, draw, ctx)
            if not state.items then
                return
            end
            local c1 = opts.color or ctx.theme.primary
            local c2 = ctx.theme.accent
            for _, p in ipairs(state.items) do
                local fg = draw:blend(c1, c2, p.hue)
                draw:cell(math.floor(p.x), math.floor(p.y), p.glyph, fg, nil)
            end
        end,
    }
end

return particles
