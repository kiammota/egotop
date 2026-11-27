-- Immediate no-op animation (implemented in Lua).
local animation = {}

function animation.setup(api)
    api.ui.reveal(1.0)
    api.finish()
end

return animation
