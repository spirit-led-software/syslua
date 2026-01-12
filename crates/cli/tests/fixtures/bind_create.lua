--- Basic bind create/destroy lifecycle.
--- Tests that binds can be created and destroyed.

local TEST_DIR = sys.getenv('TEST_OUTPUT_DIR')

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
    sys.bind({
      id = 'test-bind',
      create = function(_, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'New-Item -ItemType Directory -Force -Path "' .. TEST_DIR .. '" | Out-Null')
          sh(ctx, 'Set-Content -Path "' .. TEST_DIR .. '\\created.txt" -Value "created"')
        else
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          sh(ctx, 'echo created > ' .. TEST_DIR .. '/created.txt')
        end
        return { file = TEST_DIR .. (sys.os == 'windows' and '\\created.txt' or '/created.txt') }
      end,
      destroy = function(outputs, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'Remove-Item -Force -ErrorAction SilentlyContinue -Path "' .. outputs.file .. '"')
        else
          sh(ctx, 'rm -f ' .. outputs.file)
        end
      end,
    })
  end,
}
