-- Static no-op background effect layer (implemented in Lua).
local none = {}

function none.setup(opts)
    return {
        name = "none",
        fps = 1,
        options = opts or {},
        init = function() end,
        update = function() end,
        render = function() end,
    }
end

return none
