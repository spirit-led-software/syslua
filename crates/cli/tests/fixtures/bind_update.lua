--- Tests the update lifecycle feature.
--- Bind changes inputs, triggering an update instead of destroy+create.

local VERSION = os.getenv('TEST_VERSION')
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
      id = 'versioned-file',
      inputs = { version = VERSION },
      create = function(inputs, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'New-Item -ItemType Directory -Force -Path "' .. TEST_DIR .. '" | Out-Null')
          sh(ctx, 'Set-Content -Path "' .. TEST_DIR .. '\\version.txt" -Value "Created ' .. inputs.version .. '"')
        else
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          sh(ctx, 'echo "Created ' .. inputs.version .. '" > ' .. TEST_DIR .. '/version.txt')
        end
        return {
          file = TEST_DIR .. (sys.os == 'windows' and '\\version.txt' or '/version.txt'),
          version = inputs.version,
        }
      end,
      update = function(outputs, inputs, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'Set-Content -Path "' .. outputs.file .. '" -Value "Updated to ' .. inputs.version .. '"')
        else
          sh(ctx, 'echo "Updated to ' .. inputs.version .. '" > ' .. outputs.file)
        end
        return {
          file = outputs.file,
          version = inputs.version,
        }
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
