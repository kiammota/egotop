-- Festive 3D Procedural ASCII Christmas Tree animation (Lua Canvas / Shader API).
-- Runs cleanly isolated inside its own bordered window (`layer = "boxed"`) without leaking
-- onto neighboring panels, borders, or the status bar.
local tree_effect = require("effects.christmas_tree")

animation.type = "intro"
local animation = {}

function animation.setup(api)
    -- Keep dashboard revealed underneath while showcasing the boxed 3D Christmas tree
    if api and api.ui and api.ui.reveal then
        api.ui.reveal(1.0)
    end

    local shader = tree_effect.setup({ fps = 30, speed = 1.0 })
    return {
        name = "christmas",
        layer = "boxed",
        title = "✦ Merry Christmas — 3D ASCII ✦",
        fps = 30,
        duration = 3.8,
        init = function(ctx)
            shader.init(ctx)
        end,
        update = function(dt, t, ctx)
            shader.update(dt, t, ctx)
        end,
        render = function(canvas, ctx)
            shader.render(canvas, ctx)
        end,
    }
end

return animation
