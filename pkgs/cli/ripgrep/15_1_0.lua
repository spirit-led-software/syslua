---@class syslua.pkgs.cli.ripgrep.15_1_0
local M = {}

M.make_derivation = function()
	return derive({
		name = "ripgrep",
		version = "15.1.0",
		opts = function(system)
			-- TODO:
		end,
		config = function(opts, ctx)
			-- TODO:
		end,
	})
end

M.make_activation = function(drv)
	return activate({
		opts = { drv = drv },
		config = function(opts, ctx)
			-- TODO:
		end,
	})
end

M.setup = function()
	local derivation = M.make_derivation()
	M.make_activation(derivation)
end

return M
