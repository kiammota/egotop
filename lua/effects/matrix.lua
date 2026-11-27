-- Matrix falling glyph stream background effect (implemented in Lua).
local matrix = {}

local DEFAULT_CHARSET = "01ﾊﾐﾋｰｳｼﾅﾓﾆｻﾜﾂｵﾘｱﾎﾃﾏｹﾒｴｶｷﾑﾕﾗｾﾈｽﾀﾇﾍ"

local function split_utf8(str)
    local out = {}
    for _, code in utf8.codes(str) do
        out[#out + 1] = utf8.char(code)
    end
    return out
end

local function reset_streams(state, w, h, density)
    state.cols = {}
    local target_count = math.max(1, math.floor(w * density * 0.85))
    for i = 1, target_count do
        state.cols[i] = {
            x = math.random(0, math.max(0, w - 1)),
            y = math.random() * h,
            speed = 4.0 + math.random() * 14.0,
            len = math.random(5, math.max(6, math.floor(h * 0.65))),
            glyphs = {},
        }
    end
end

function matrix.setup(opts)
    opts = opts or {}
    local glyphs = split_utf8(opts.charset or DEFAULT_CHARSET)
    local density = math.max(0.05, math.min(1.0, opts.density or 0.6))
    local speed_scale = opts.speed or 1.0

    return {
        name = "matrix",
        fps = opts.fps or 24,
        options = opts,
        init = function(state, ctx)
            state.tick = 0
            reset_streams(state, ctx.width, ctx.height, density)
        end,
        resize = function(state, ctx)
            reset_streams(state, ctx.width, ctx.height, density)
        end,
        update = function(state, dt, ctx)
            state.tick = (state.tick or 0) + dt
            if not state.cols then
                reset_streams(state, ctx.width, ctx.height, density)
            end
            for _, col in ipairs(state.cols) do
                col.y = col.y + col.speed * speed_scale * dt
                if col.y - col.len > ctx.height then
                    col.y = -math.random(0, 6)
                    col.x = math.random(0, math.max(0, ctx.width - 1))
                end
            end
        end,
        render = function(state, draw, ctx)
            local head_color = opts.head_color or ctx.theme.text
            local trail_color = opts.color or ctx.theme.primary
            local tail_color = ctx.theme.muted
            local n_glyphs = #glyphs
            if n_glyphs == 0 or not state.cols then
                return
            end

            for _, col in ipairs(state.cols) do
                local head_y = math.floor(col.y)
                for offset = 0, col.len - 1 do
                    local y = head_y - offset
                    if y >= 0 and y < ctx.height then
                        local idx = ((col.x * 31 + y * 17 + math.floor(state.tick * 8)) % n_glyphs) + 1
                        local ch = glyphs[idx]
                        local fg
                        if offset == 0 then
                            fg = head_color
                        else
                            local t = offset / math.max(1, col.len - 1)
                            fg = draw:blend(trail_color, tail_color, t)
                        end
                        draw:cell(col.x, y, ch, fg, nil)
                    end
                end
            end
        end,
    }
end

return matrix
