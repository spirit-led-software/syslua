--- Build that executes cross-platform commands.
--- Tests shell execution within the sandbox environment.

--- Cross-platform shell execution with PATH injection for sandbox.
--- @param ctx ActionCtx
--- @param script string
--- @return string
local function sh(ctx, script)
  if sys.os == 'windows' then
    return ctx:exec({ bin = 'cmd.exe', args = { '/c', script } })
  else
    return ctx:exec({
      bin = '/bin/sh',
      args = { '-c', script },
      env = { PATH = '/bin:/usr/bin' },
    })
  end
end

return {
  inputs = {},
  setup = function(_)
    sys.build({
      id = 'hello-1.0.0',
      create = function(_, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'mkdir ' .. ctx.out .. '\\bin')
          sh(ctx, 'echo @echo Hello > ' .. ctx.out .. '\\bin\\hello.cmd')
        else
          sh(ctx, 'mkdir -p ' .. ctx.out .. '/bin')
          sh(ctx, 'printf "#!/bin/sh\\necho Hello\\n" > ' .. ctx.out .. '/bin/hello')
          sh(ctx, 'chmod +x ' .. ctx.out .. '/bin/hello')
        end
        return { bin = ctx.out .. '/bin/hello' .. (sys.os == 'windows' and '.cmd' or '') }
      end,
    })
  end,
}
