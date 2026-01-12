--- Tests that dependent binds are skipped when a build fails.
---
--- Expected behavior:
--- 1. 'failing-build' fails during create
--- 2. 'depends-on-failing-build' bind is skipped (not executed)
--- 3. No file should be created at TEST_OUTPUT_DIR/should-not-exist.txt

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
    local build = sys.build({
      id = 'failing-build',
      create = function(_, ctx)
        sh(ctx, 'exit 1') -- deliberate failure
        return { out = ctx.out }
      end,
    })

    -- This bind depends on the failing build, should be skipped
    sys.bind({
      id = 'depends-on-failing-build',
      inputs = { build = build },
      create = function(_, ctx)
        if sys.os == 'windows' then
          sh(ctx, 'New-Item -ItemType Directory -Force -Path "' .. TEST_DIR .. '" | Out-Null')
          sh(ctx, 'Set-Content -Path "' .. TEST_DIR .. '\\should-not-exist.txt" -Value "should not exist"')
        else
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          sh(ctx, 'touch ' .. TEST_DIR .. '/should-not-exist.txt')
        end
        return {}
      end,
      destroy = function(_, _) end,
    })
  end,
}
