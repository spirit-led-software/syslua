--- Build-only config (no binds).
--- Tests that builds work without requiring binds.

--- Cross-platform shell execution with PATH injection for sandbox.
--- @param ctx BuildCtx | BindCtx
--- @param script string
--- @return string
local function sh(ctx, script)
  if sys.os == 'windows' then
    local system_drive = os.getenv('SystemDrive') or 'C:'
    return ctx:exec({
      bin = 'powershell.exe',
      args = {
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        script,
      },
      env = { PATH = system_drive .. '\\Windows\\System32;' .. system_drive .. '\\Windows' },
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
      id = 'simple-build-1.0.0',
      create = function(_, ctx)
        -- Create a simple file in the output directory
        if sys.os == 'windows' then
          sh(ctx, 'Set-Content -Path "' .. ctx.out .. '\\hello.txt" -Value "hello"')
        else
          sh(ctx, 'echo hello > ' .. ctx.out .. '/hello.txt')
        end
        return { out = ctx.out }
      end,
    })
  end,
}
