---@class syslua.pkgs
---@field cli syslua.pkgs.cli
local M = {}

setmetatable(M, {
	__index = function(_, pkg)
		return require("syslua.pkgs." .. pkg)
	end,
})

return M
