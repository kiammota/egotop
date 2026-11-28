-- High-density procedural 3D rotating ASCII Christmas tree (Lua Canvas / Cell-Buffer API).
-- Uses truecolor lighting, z-buffered projection (`canvas:put_z`), sinusoidal lights,
-- wrapped gifts at the base, and parallax falling snow strictly clipped to the canvas.
local tree = {}

tree.type = "canvas"

local FOLIAGE_RAMP = { ".", ":", "+", "*", "%", "#", "@" }

local ORNAMENT_COLORS = {
    "#FF3B5C", -- ruby red
    "#FFD166", -- warm gold
    "#4CC9F0", -- icy cyan
    "#F72585", -- festive magenta
    "#80FFDB", -- mint glow
    "#FF9F1C", -- tangerine
}

local function seed_snow(state, w, h, count)
    state.snow = {}
    local n = math.max(12, count or math.floor((w * h) / 26))
    for i = 1, n do
        local depth = (i % 3) + 1
        state.snow[i] = {
            x = math.random() * math.max(1, w),
            y = math.random() * math.max(1, h),
            vy = (1.6 + depth * 1.1) * (0.85 + math.random() * 0.35),
            sway = math.random() * math.pi * 2.0,
            depth = depth,
            ch = (depth == 3 and "*") or (depth == 2 and "·") or ".",
        }
    end
end

local function draw_gift(canvas, cx, base_y, w, h, z_front, box_col, ribbon_col)
    local x0 = math.floor(cx - w / 2)
    local y0 = math.floor(base_y - h + 1)
    local mid_x = math.floor(x0 + w / 2)
    local mid_y = math.floor(y0 + h / 2)
    for dy = 0, h - 1 do
        for dx = 0, w - 1 do
            local ch = "█"
            local col = box_col
            local inten = 0.85
            if dy == 0 then
                ch = (dx == mid_x - x0) and "✦" or "▀"
                col = (dx == mid_x - x0) and ribbon_col or box_col
                inten = 1.0
            elseif dx == mid_x - x0 or dy == mid_y - y0 then
                ch = (dx == mid_x - x0 and dy == mid_y - y0) and "+" or (dx == mid_x - x0 and "┃" or "━")
                col = ribbon_col
                inten = 1.05
            else
                ch = ((dx + dy) % 2 == 0) and "#" or "%"
                inten = 0.72
            end
            canvas:put_z(x0 + dx, y0 + dy, z_front, ch, col, nil, inten)
        end
    end
end

function tree.setup(opts)
    opts = opts or {}
    local speed = opts.speed or 1.0
    local snow_count = opts.snow_count

    return {
        name = "christmas_tree",
        fps = opts.fps or 30,
        options = opts,
        init = function(ctx)
            seed_snow(ctx.state, ctx.width, ctx.height, snow_count)
        end,
        resize = function(ctx)
            seed_snow(ctx.state, ctx.width, ctx.height, snow_count)
        end,
        update = function(dt, _t, ctx)
            local state = ctx.state
            if not state.snow then
                seed_snow(state, ctx.width, ctx.height, snow_count)
            end
            for _, f in ipairs(state.snow) do
                f.y = f.y + f.vy * speed * dt
                f.x = (f.x + ctx.sin(ctx.t * 1.4 + f.sway) * 1.6 * dt) % math.max(1, ctx.width)
                if f.y >= ctx.height then
                    f.y = 0
                    f.x = math.random() * math.max(1, ctx.width)
                end
            end
        end,
        render = function(canvas, ctx)
            local w = canvas.width
            local h = canvas.height
            if w < 10 or h < 8 then
                return
            end

            local t = ctx.t * speed
            local angle = t * 0.85

            -- 1. Background snowfall & distant night stars (drawn via put, keeping Z-buffer free for the 3D tree)
            if ctx.state.snow then
                for _, f in ipairs(ctx.state.snow) do
                    local shimmer = 0.45 + 0.55 * (0.5 + 0.5 * ctx.sin(t * 2.2 + f.sway))
                    local depth_k = 0.35 + (f.depth / 3.0) * 0.55
                    canvas:put(f.x, f.y, f.ch, "#DCEEFF", nil, depth_k * shimmer)
                end
            end

            -- 2. Tree dimensional proportions fitted strictly inside canvas bounds
            local tree_top = 2
            local ground_y = h - 2
            local trunk_h = math.max(2, math.floor(h * 0.14))
            local canopy_h = math.max(5, ground_y - trunk_h - tree_top)
            local cx = (w - 1) * 0.5
            local max_r = math.min((w - 4) * 0.24, canopy_h * 0.58)
            local aspect_x = 2.05

            -- Ground snowdrift carpet
            for gx = 0, w - 1 do
                local wave = ctx.sin(gx * 0.35 + 1.2)
                local ch = (wave > 0.35 and "~") or (wave < -0.35 and ".") or "_"
                canvas:put(gx, ground_y + 1, ch, "#B8D8EB", nil, 0.55)
            end

            -- Directional key light (front upper-right)
            local lx, ly, lz = 0.42, -0.32, 0.85
            local ca = ctx.cos(angle)
            local sa = ctx.sin(angle)

            -- 3. 3D Trunk cylinder
            local trunk_r = math.max(0.85, max_r * 0.14)
            for ty = 0, trunk_h - 1 do
                local py = tree_top + canopy_h + ty
                local steps = 16
                for s = 0, steps - 1 do
                    local th = (s / steps) * math.pi * 2.0
                    local rx = trunk_r * ctx.cos(th)
                    local rz = trunk_r * ctx.sin(th)
                    local px = math.floor(cx + rx * aspect_x + 0.5)
                    if rz >= -trunk_r * 0.3 then
                        local light = ctx.clamp(0.35 + 0.65 * (rz / trunk_r), 0.25, 1.0)
                        local ch = light > 0.72 and "#" or (light > 0.48 and "|" or ":")
                        canvas:put_z(px, py, rz, ch, "#9B5E2E", nil, light)
                    end
                end
            end

            -- 4. High-density 3D Foliage Cone (multi-tiered scalloped pine boughs with z-buffer shading)
            local v_samples = math.max(canopy_h * 2, 24)
            for vi = 0, v_samples do
                local yn = vi / v_samples -- 0 = tip, 1 = base
                local py = tree_top + yn * canopy_h
                -- Pine bough tiers create realistic conical skirt waves
                local tier = (yn * 4.5) % 1.0
                local r_base = max_r * (0.12 + 0.88 * yn) * (0.72 + 0.32 * tier)
                local theta_steps = math.max(18, math.floor(r_base * 10.0))

                for ti = 0, theta_steps - 1 do
                    local th = (ti / theta_steps) * math.pi * 2.0 + yn * 1.7
                    local lobe = 0.84 + 0.16 * ctx.sin(th * 6.0 + yn * 8.0)
                    local r = r_base * lobe

                    local x3 = r * ctx.cos(th)
                    local z3 = r * ctx.sin(th)

                    -- Y-axis rotation
                    local rx = x3 * ca - z3 * sa
                    local rz = x3 * sa + z3 * ca

                    -- Simple perspective scale
                    local persp = 1.0 / (1.0 - rz * 0.018)
                    local sx = cx + rx * aspect_x * persp
                    local sy = py + rz * 0.08

                    -- Surface normal & Lambertian intensity
                    local inv_r = 1.0 / math.max(0.01, math.sqrt(rx * rx + rz * rz + 0.25))
                    local nx = rx * inv_r * 0.88
                    local ny = -0.38
                    local nz = rz * inv_r * 0.88
                    local ndotl = ctx.clamp(nx * lx + ny * ly + nz * lz, 0.0, 1.0)
                    local depth_norm = ctx.clamp((rz / math.max(1.0, max_r) + 1.0) * 0.5, 0.0, 1.0)
                    local shade = ctx.clamp(0.18 + 0.58 * ndotl + 0.24 * depth_norm, 0.12, 1.0)

                    local ramp_idx = math.min(#FOLIAGE_RAMP, math.max(1, math.floor(shade * (#FOLIAGE_RAMP - 0.01)) + 1))
                    local ch = FOLIAGE_RAMP[ramp_idx]

                    -- Truecolor green gradient: dark emerald interior -> vibrant spruce -> frosted needle tips
                    local foliage_rgb = ctx.rgb_lerp("#0C4426", "#27B863", yn * 0.45 + tier * 0.55)
                    if shade > 0.82 then
                        foliage_rgb = ctx.rgb_lerp(foliage_rgb, "#A8F7C6", (shade - 0.82) * 3.5)
                    end

                    canvas:put_z(sx, sy, rz, ch, foliage_rgb, nil, 0.42 + 0.65 * shade)
                end
            end

            -- 5. Rotating spiral garland + sinusoidal blinking 3D ornaments
            local orb_count = math.max(18, math.floor(canopy_h * 1.45))
            for oi = 1, orb_count do
                local yn = 0.14 + 0.82 * ((oi - 1) / math.max(1, orb_count - 1))
                local tier = (yn * 4.5) % 1.0
                local r = max_r * (0.14 + 0.88 * yn) * (0.74 + 0.30 * tier) * 1.02
                local th = oi * 2.39996 + yn * 5.0
                local x3 = r * ctx.cos(th)
                local z3 = r * ctx.sin(th)
                local rx = x3 * ca - z3 * sa
                local rz = x3 * sa + z3 * ca
                if rz > -max_r * 0.15 then
                    local persp = 1.0 / (1.0 - rz * 0.018)
                    local sx = cx + rx * aspect_x * persp
                    local sy = tree_top + yn * canopy_h + rz * 0.08
                    local blink = 0.5 + 0.5 * ctx.sin(t * (2.6 + (oi % 3) * 0.9) + oi * 1.37)
                    local col = ORNAMENT_COLORS[(oi % #ORNAMENT_COLORS) + 1]
                    local ch = blink > 0.72 and "◉" or (blink > 0.38 and "●" or "o")
                    canvas:put_z(sx, sy, rz + 0.35, ch, col, nil, 0.55 + 0.65 * blink)
                end
            end

            -- 6. Shining animated star topper
            local star_y = tree_top - 1
            local star_pulse = 0.5 + 0.5 * ctx.sin(t * 4.2)
            local star_col = ctx.rgb_lerp("#FFB703", "#FFF6B3", star_pulse)
            local halo_col = ctx.rgb_lerp("#FF9F1C", "#FFE66D", star_pulse)
            local z_star = max_r + 2.0
            canvas:put_z(cx, star_y, z_star + 0.2, "★", star_col, nil, 1.15)
            if star_y - 1 >= 0 then
                canvas:put_z(cx, star_y - 1, z_star, star_pulse > 0.5 and "|" or "·", halo_col, nil, 0.75 + 0.25 * star_pulse)
            end
            canvas:put_z(cx - 1, star_y, z_star, star_pulse > 0.45 and "-" or "·", halo_col, nil, 0.8)
            canvas:put_z(cx + 1, star_y, z_star, star_pulse > 0.45 and "-" or "·", halo_col, nil, 0.8)

            -- 7. Wrapped gift boxes around the base
            if w >= 26 and trunk_h >= 2 then
                local g_y = ground_y
                local gift_r = math.max(3.5, max_r * 0.68) * aspect_x
                draw_gift(canvas, cx - gift_r * 0.75, g_y, 5, 2, max_r + 0.5, "#E63946", "#FFD166")
                draw_gift(canvas, cx + gift_r * 0.75, g_y, 5, 2, max_r + 0.5, "#3A86FF", "#FFE66D")
                if w >= 38 then
                    draw_gift(canvas, cx - gift_r * 0.32, g_y, 3, 2, max_r + 0.8, "#9B5DE5", "#80FFDB")
                end
            end
        end,
    }
end

return tree
