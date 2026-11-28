-- Immediate no-op animation (implemented in Lua).
local animation = {}

animation.type = "intro"

function animation.setup(api)
    api.ui.reveal(1.0)
    api.finish()
end

return animation
