--- Atomic apply with rollback test configuration
---
--- Demonstrates the rollback behavior when apply fails after destroying binds.
--- When execution fails, previously destroyed binds are restored to their
--- original state, and the snapshot pointer is restored to the previous state.
---
--- Usage:
---   # Set up the test environment
---   mkdir -p /tmp/syslua-rollback-test
---
---   # First apply - creates initial state (binds A and B)
---   ROLLBACK_PHASE=initial sys apply tests/configs/rollback.lua
---
---   # Verify initial state
---   ls -la /tmp/syslua-rollback-test/
---   # Should show: file_a.txt, file_b.txt
---
---   # Second apply - simulates removing bind A and adding a failing bind C
---   # This should:
---   # 1. Destroy bind A (removes file_a.txt)
---   # 2. Try to apply bind C (which fails)
---   # 3. Restore bind A (recreates file_a.txt)
---   # 4. Leave system in original state
---   ROLLBACK_PHASE=failure sys apply tests/configs/rollback.lua
---
---   # Verify rollback occurred
---   ls -la /tmp/syslua-rollback-test/
---   # Should still show: file_a.txt, file_b.txt (bind A was restored)
---
---   # Third apply - successful modification
---   # Removes bind A, adds bind C that succeeds
---   ROLLBACK_PHASE=success sys apply tests/configs/rollback.lua
---
---   # Verify successful apply
---   ls -la /tmp/syslua-rollback-test/
---   # Should show: file_b.txt, file_c.txt (bind A removed, C added)
---
---   # Cleanup
---   rm -rf /tmp/syslua-rollback-test
---
--- Environment Variables:
---   ROLLBACK_PHASE: Controls which phase to run
---     - "initial": Create initial state with binds A and B
---     - "failure": Try to remove A and add failing C (should rollback)
---     - "success": Remove A and add successful C
---
--- Expected Behavior:
---   1. Initial apply creates file_a.txt and file_b.txt
---   2. Failure apply destroys file_a.txt, fails on C, restores file_a.txt
---   3. Success apply removes file_a.txt, creates file_c.txt

local TEST_DIR = '/tmp/syslua-rollback-test'

--- Execute a shell command (cross-platform)
--- @param ctx ActionCtx
--- @param script string
--- @return string
local function sh(ctx, script)
  if package.config:sub(1, 1) == '\\' then
    -- Windows: use cmd.exe
    return ctx:exec({ bin = 'cmd.exe', args = { '/c', script } })
  else
    -- Unix: use /bin/sh
    return ctx:exec({ bin = '/bin/sh', args = { '-c', script } })
  end
end

-- Get phase from environment, default to "initial"
local phase = os.getenv('ROLLBACK_PHASE') or 'initial'

return {
  inputs = {},
  setup = function()
    -- Bind A: Only present in "initial" phase
    -- Will be destroyed in "failure" and "success" phases
    if phase == 'initial' then
      sys.bind({
        outputs = { file = TEST_DIR .. '/file_a.txt' },
        apply = function(_, ctx)
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          sh(ctx, 'echo "Content A - created at $(date)" > ' .. TEST_DIR .. '/file_a.txt')
          return { file = TEST_DIR .. '/file_a.txt' }
        end,
        destroy = function(outputs, ctx)
          sh(ctx, 'rm -f ' .. outputs.file)
        end,
      })
    end

    -- Bind B: Always present (unchanged across all phases)
    -- This should never be affected by rollback
    sys.bind({
      outputs = { file = TEST_DIR .. '/file_b.txt' },
      apply = function(_, ctx)
        sh(ctx, 'mkdir -p ' .. TEST_DIR)
        sh(ctx, 'echo "Content B - created at $(date)" > ' .. TEST_DIR .. '/file_b.txt')
        return { file = TEST_DIR .. '/file_b.txt' }
      end,
      destroy = function(outputs, ctx)
        sh(ctx, 'rm -f ' .. outputs.file)
      end,
    })

    -- Bind C: Only in "failure" and "success" phases
    -- In "failure" phase, this bind will fail
    -- In "success" phase, this bind will succeed
    if phase == 'failure' then
      sys.bind({
        outputs = { file = TEST_DIR .. '/file_c.txt' },
        apply = function(_, ctx)
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          -- This command will fail with exit code 1
          sh(ctx, 'echo "About to fail..." && exit 1')
          return { file = TEST_DIR .. '/file_c.txt' }
        end,
        destroy = function(outputs, ctx)
          sh(ctx, 'rm -f ' .. outputs.file)
        end,
      })
    elseif phase == 'success' then
      sys.bind({
        outputs = { file = TEST_DIR .. '/file_c.txt' },
        apply = function(_, ctx)
          sh(ctx, 'mkdir -p ' .. TEST_DIR)
          sh(ctx, 'echo "Content C - created at $(date)" > ' .. TEST_DIR .. '/file_c.txt')
          return { file = TEST_DIR .. '/file_c.txt' }
        end,
        destroy = function(outputs, ctx)
          sh(ctx, 'rm -f ' .. outputs.file)
        end,
      })
    end
  end,
}
