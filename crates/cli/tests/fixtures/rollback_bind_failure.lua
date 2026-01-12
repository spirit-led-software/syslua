--- Tests rollback when a bind fails after destroying previous binds.
---
--- Test flow:
--- 1. First apply with TEST_PHASE=initial creates 'original-bind'
--- 2. Second apply with TEST_PHASE=failure removes 'original-bind' and adds 'failing-bind'
--- 3. 'failing-bind' fails during create
--- 4. Rollback should restore 'original-bind'

local TEST_DIR = sys.getenv('TEST_OUTPUT_DIR')
local PHASE = os.getenv('TEST_PHASE')

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
    if PHASE == 'initial' then
      -- This bind will be destroyed on second apply
      sys.bind({
        id = 'original-bind',
        create = function(_, ctx)
          if sys.os == 'windows' then
            sh(ctx, 'New-Item -ItemType Directory -Force -Path "' .. TEST_DIR .. '" | Out-Null')
            sh(ctx, 'Set-Content -Path "' .. TEST_DIR .. '\\original.txt" -Value "original"')
          else
            sh(ctx, 'mkdir -p ' .. TEST_DIR)
            sh(ctx, 'echo original > ' .. TEST_DIR .. '/original.txt')
          end
          return { file = TEST_DIR .. (sys.os == 'windows' and '\\original.txt' or '/original.txt') }
        end,
        destroy = function(outputs, ctx)
          if sys.os == 'windows' then
            sh(ctx, 'Remove-Item -Force -ErrorAction SilentlyContinue -Path "' .. outputs.file .. '"')
          else
            sh(ctx, 'rm -f ' .. outputs.file)
          end
        end,
      })
    elseif PHASE == 'failure' then
      -- This bind will fail during create
      sys.bind({
        id = 'failing-bind',
        create = function(_, ctx)
          sh(ctx, 'exit 1') -- deliberate failure
          return {}
        end,
        destroy = function(_, _) end,
      })
    end
  end,
}
