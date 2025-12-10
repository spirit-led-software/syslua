---@class syslua.pkgs.cli
---@field ripgrep syslua.pkgs.cli.ripgrep
local M = {}

setmetatable(M, {
	__index = function(_, pkg)
		return require("pkgs.cli." .. pkg)
	end,
})

return M
