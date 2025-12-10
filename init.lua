---@class syslua
---@field pkgs syslua.pkgs
---@field lib syslua.lib
---@field modules syslua.modules
local M = {}

setmetatable(M, {
	__index = function(_, module)
		return require("syslua." .. module)
	end,
})

return M
