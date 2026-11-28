-- Cyber boot sequence: scans a neon laser bar vertically inside an isolated canvas.
local animation = {}

animation.type = "intro"

function animation.setup(api)
    local total = 1.25
    api.ui.reveal(0.0)

    return {
        name = "cyber_boot",
        description = "Cyber laser scan boot sequence on an isolated canvas",
        fps = 30,
        duration = total,
        render = function(canvas, ctx)
            local w, h = canvas.width, canvas.height
            if w <= 0 or h <= 0 then
                return
            end
            local t = ctx.t
            local badge = "[ cyberlink online ]"
            local bx = math.max(0, math.floor((w - #badge) / 2))
            local by = math.floor(h / 2)

            if t < 0.9 then
                local p = math.min(1.0, t / 0.9)
                ctx.reveal(p * 0.75)
                local beam_y = math.min(h - 1, math.floor(p * math.max(0, h - 1)))
                canvas:text(0, beam_y, ("━"):rep(w), "#00FFD5", nil, 1.0)
                canvas:text(bx, by, badge, "#FF2A85", nil, 1.0)
            else
                local p = math.min(1.0, (t - 0.9) / 0.35)
                ctx.reveal(0.75 + p * 0.25)
                local col = ctx.rgb_lerp("#FF2A85", "#00FFD5", p)
                canvas:text(bx, by, badge, col, nil, 1.0)
            end
        end,
    }
end

return animation
