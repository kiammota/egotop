-- Shared helper for EgoTop Lua themes to merge user overrides cleanly
-- without mutating the base theme module.
local M = {}

function M.merge(base, overrides)
    local out = {}
    for k, v in pairs(base) do
        if type(v) == "table" then
            local cp = {}
            for sk, sv in pairs(v) do
                cp[sk] = sv
            end
            out[k] = cp
        else
            out[k] = v
        end
    end
    if type(overrides) == "table" then
        for k, v in pairs(overrides) do
            if type(v) == "table" and type(out[k]) == "table" then
                for sk, sv in pairs(v) do
                    out[k][sk] = sv
                end
            else
                out[k] = v
            end
        end
    end
    return out
end

return M
