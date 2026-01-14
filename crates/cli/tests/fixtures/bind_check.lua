--- Bind with check callback for drift detection tests.
--- Tests that check callbacks detect drift when files are modified/deleted.

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
      id = 'check-test',
      create = function(_, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'New-Item -ItemType Directory -Force -Path "' .. TEST_DIR .. '" | Out-Null')
          sh(ctx, 'Set-Content -Path "' .. TEST_DIR .. '\\check-marker.txt" -Value "exists"')
        else
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          sh(ctx, 'echo exists > ' .. TEST_DIR .. '/check-marker.txt')
        end
        return { file = TEST_DIR .. (sys.os == 'windows' and '\\check-marker.txt' or '/check-marker.txt') }
      end,
      check = function(outputs, _, ctx)
        local drifted
        if sys.os == 'windows' then
          drifted = sh(
            ctx,
            'if (Test-Path "'
              .. outputs.file
              .. '") { Write-Host -NoNewline "false" } else { Write-Host -NoNewline "true" }'
          )
        else
          drifted = sh(ctx, 'test -f "' .. outputs.file .. '" && printf false || printf true')
        end
        return { drifted = drifted, message = 'file does not exist' }
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
