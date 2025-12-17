--- Placeholder test configuration
--- Demonstrates the $${...} placeholder syntax for deferred value resolution
---
--- Key features demonstrated:
--- 1. Action chaining - ctx:fetch_url() returns $${action:0}, used in sh()
--- 2. Multiple outputs - Build returning multiple named outputs
--- 3. Build -> Bind references - Using build outputs in bind commands
--- 4. Shell variables - $HOME, $PATH passing through unchanged (no escaping needed)
--- 5. Destroy actions - Cleanup commands for rollback

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

return {
  inputs = {},
  setup = function(_)
    -- Build ripgrep from release tarball
    -- Demonstrates action chaining: fetch returns $${action:0}, used by extract command
    -- Note: Using a real release URL that contains the binary
    local rg = sys.build({
      name = 'ripgrep',
      version = '14.1.1',
      apply = function(_, ctx)
        -- fetch_url returns $${action:0} - the download location
        local archive = ctx:fetch_url(
          'https://github.com/BurntSushi/ripgrep/releases/download/14.1.1/ripgrep-14.1.1-x86_64-apple-darwin.tar.gz',
          'fc87e78f7cb3fea12d69072e7ef3b21509754717b746368fd40d88963630e2b3'
        )

        -- Create output directories using ctx.out (resolves to $${out}, the build's store path)
        sh(ctx, 'mkdir -p ' .. ctx.out .. '/bin ' .. ctx.out .. '/share/man/man1')

        -- Extract to TMPDIR (automatically set by syslua to a clean temp space)
        -- Also demonstrates shell variable $TMPDIR passing through unchanged
        sh(ctx, 'tar xf ' .. archive .. ' -C $TMPDIR')

        -- Copy the binary and man page to output using ctx.out
        sh(
          ctx,
          'cp $TMPDIR/ripgrep-14.1.1-x86_64-apple-darwin/rg '
            .. ctx.out
            .. '/bin/ && '
            .. 'cp $TMPDIR/ripgrep-14.1.1-x86_64-apple-darwin/doc/rg.1 '
            .. ctx.out
            .. '/share/man/man1/'
        )

        -- Return multiple named outputs using ctx.out
        return {
          out = ctx.out,
          bin = ctx.out .. '/bin/rg',
          man = ctx.out .. '/share/man/man1/rg.1',
        }
      end,
    })

    -- Build fd (another CLI tool)
    -- Demonstrates simpler build with environment variables
    local fd = sys.build({
      name = 'fd',
      version = '10.2.0',
      apply = function(_, ctx)
        local archive = ctx:fetch_url(
          'https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-x86_64-apple-darwin.tar.gz',
          '991a648a58870230af9547c1ae33e72cb5c5199a622fe5e540e162d6dba82d48'
        )

        sh(ctx, 'mkdir -p ' .. ctx.out .. '/bin')
        sh(ctx, 'tar xf ' .. archive .. ' -C $TMPDIR')
        sh(ctx, 'cp $TMPDIR/fd-v10.2.0-x86_64-apple-darwin/fd ' .. ctx.out .. '/bin/')

        return { out = ctx.out }
      end,
    })

    -- Bind ripgrep to system
    -- Demonstrates using build outputs in bind commands
    sys.bind({
      inputs = { rg = rg },
      apply = function(bind_inputs, ctx)
        -- Create target directory first
        sh(ctx, 'mkdir -p /tmp/syslua-test/.local/bin /tmp/syslua-test/.local/share/man/man1')

        -- Reference build output via inputs
        sh(ctx, 'ln -sf ' .. bind_inputs.rg.outputs.bin .. ' /tmp/syslua-test/.local/bin/rg')

        -- Create man page symlink using build outputs
        sh(ctx, 'ln -sf ' .. bind_inputs.rg.outputs.man .. ' /tmp/syslua-test/.local/share/man/man1/rg.1')
      end,
      destroy = function(_, ctx)
        -- Cleanup commands
        sh(ctx, 'rm -f /tmp/syslua-test/.local/bin/rg')
        sh(ctx, 'rm -f /tmp/syslua-test/.local/share/man/man1/rg.1')
      end,
    })

    -- Bind fd to system
    sys.bind({
      inputs = { fd = fd },
      apply = function(bind_inputs, ctx)
        sh(ctx, 'mkdir -p /tmp/syslua-test/.local/bin')
        sh(ctx, 'ln -sf ' .. bind_inputs.fd.outputs.out .. '/bin/fd /tmp/syslua-test/.local/bin/fd')
      end,
      destroy = function(_, ctx)
        sh(ctx, 'rm -f /tmp/syslua-test/.local/bin/fd')
      end,
    })

    -- Bind that creates an env file combining multiple builds
    -- Demonstrates referencing multiple builds and complex shell variable usage
    sys.bind({
      inputs = { rg = rg, fd = fd },
      apply = function(bind_inputs, ctx)
        sh(ctx, 'mkdir -p /tmp/syslua-test/.local')

        -- Create an env.sh that sets up PATH with both tools
        -- Note: $PATH at end is a shell variable (preserved)
        -- The build output paths are Lua string concatenation
        local env_content = 'export PATH='
          .. bind_inputs.rg.outputs.out
          .. '/bin:'
          .. bind_inputs.fd.outputs.out
          .. '/bin:$PATH'

        sh(ctx, 'echo "' .. env_content .. '" > /tmp/syslua-test/.local/env.sh')
      end,
      destroy = function(_, ctx)
        sh(ctx, 'rm -f /tmp/syslua-test/.local/env.sh')
      end,
    })
  end,
}
