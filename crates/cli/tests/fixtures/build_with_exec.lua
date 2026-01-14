--- Build that executes cross-platform commands.
--- Tests shell execution within the sandbox environment.

--- Cross-platform shell execution with PATH injection for sandbox.
--- @param ctx BuildCtx | BindCtx
--- @param script string
--- @return string
local function sh(ctx, script)
  if sys.os == 'windows' then
    return ctx:exec({
      bin = 'powershell.exe',
      args = { '-NoProfile', '-NonInteractive', '-Command', script },
      env = { PATH = sys.getenv('SystemDrive') .. '\\Windows\\System32;' .. sys.getenv('SystemDrive') .. '\\Windows' },
    })
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
          sh(ctx, 'New-Item -ItemType Directory -Force -Path "' .. ctx.out .. '\\bin" | Out-Null')
          sh(ctx, 'Set-Content -Path "' .. ctx.out .. '\\bin\\hello.cmd" -Value "@echo Hello"')
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
