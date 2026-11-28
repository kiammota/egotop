-- Resolution independent normalized-vector canvas demo.
local delorean = { type = "canvas" }

function delorean.setup(opts)
    opts = opts or {}
    return {
        name = "delorean",
        fps = opts.fps or 24,
        init = function(ctx)
            ctx.state.sparkle_phase = ctx.t or 0
        end,
        on_resize = function(_width, _height, ctx)
            -- Keep phase and state intact; geometry is entirely normalized.
            ctx.state.last_aspect = ctx.aspect
        end,
        update = function(dt, t, ctx)
            ctx.state.sparkle_phase = t
        end,
        render = function(canvas, ctx)
            local pulse = 0.5 + 0.5 * ctx.sin(ctx.t * 3.0)
            local metal = ctx.rgb_lerp("#8496A8", "#E7F2FF", pulse)
            -- Fit a fixed 2:1 physical silhouette into any canvas aspect ratio.
            local car_h = math.min(0.55, 0.84 * math.max(0.1, ctx.aspect) / 2.0)
            local car_w = car_h * 2.0 / math.max(0.1, ctx.aspect)
            local function car(x, y)
                return 0.5 + (x - 0.5) * car_w / 0.83,
                    0.58 + (y - 0.59) * car_h / 0.46
            end
            local function line(x1, y1, x2, y2, color)
                local u1, v1 = car(x1, y1); local u2, v2 = car(x2, y2)
                canvas:line_uv(u1, v1, u2, v2, color)
            end
            -- Body profile, cabin, front/rear spoilers and wheels scale to the panel.
            line(0.12, 0.70, 0.22, 0.57, metal)
            line(0.22, 0.57, 0.68, 0.57, metal)
            line(0.68, 0.57, 0.88, 0.68, metal)
            line(0.88, 0.68, 0.93, 0.78, "#35D8FF")
            line(0.93, 0.78, 0.10, 0.78, "#35D8FF")
            line(0.10, 0.78, 0.12, 0.70, metal)
            line(0.34, 0.56, 0.42, 0.36, "#A9E8FF")
            line(0.42, 0.36, 0.64, 0.36, "#A9E8FF")
            line(0.64, 0.36, 0.76, 0.59, "#A9E8FF")
            line(0.40, 0.60, 0.40, 0.39, "#587B92")
            line(0.63, 0.59, 0.63, 0.38, "#587B92")
            for _, x in ipairs({0.28, 0.75}) do
                local u, v = car(x, 0.77)
                canvas:circle_uv(u, v, 0.085 * car_h / 0.46, "#101820")
                canvas:circle_uv(u, v, 0.035 * car_h / 0.46, "#E6F5FF")
            end
            line(0.15, 0.82, 0.88, 0.82, "#FFB703")
            if (ctx.lod or 0) > 0.25 then
                for i = 1, math.floor(12 * ctx.lod) do
                    local u = (i * 0.618033 + ctx.t * 0.015) % 1
                    local v = (i * 0.371) % 0.28
                    canvas:dot_uv(u, v, "#B7E9FF")
                end
            end
        end,
    }
end

return delorean
