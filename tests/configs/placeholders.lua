--- Placeholder test configuration
--- Demonstrates the $${...} placeholder syntax for deferred value resolution
---
--- Key features demonstrated:
--- 1. Action chaining - ctx:fetch_url() returns $${action:0}, used in ctx:cmd()
--- 2. Multiple outputs - Build returning multiple named outputs
--- 3. Build -> Bind references - Using build outputs in bind commands
--- 4. Shell variables - $HOME, $PATH passing through unchanged (no escaping needed)
--- 5. Destroy actions - Cleanup commands for rollback
return {
  inputs = {},
  setup = function(inputs)
    -- Build ripgrep from source
    -- Demonstrates action chaining: fetch returns $${action:0}, used by extract command
    local rg = sys.build({
      name = "ripgrep",
      version = "14.1.0",
      apply = function(build_inputs, ctx)
        -- fetch_url returns $${action:0} - the download location
        local archive = ctx:fetch_url(
          "https://github.com/BurntSushi/ripgrep/releases/download/14.1.0/ripgrep-14.1.0.tar.gz",
          "abc123def456"
        )

        -- Use $${action:0} in command - will be substituted with actual path at runtime
        -- Also demonstrates shell variable $PWD passing through unchanged
        ctx:cmd({ cmd = "echo 'Extracting to $PWD' && tar xf " .. archive .. " -C /build" })

        -- Build step uses $PATH shell variable naturally (no escaping needed)
        ctx:cmd({ cmd = "cd /build/ripgrep-14.1.0 && PATH=$PATH:/extra/bin make install DESTDIR=/out" })

        -- Return multiple named outputs
        return {
          out = "/out",
          bin = "/out/bin/rg",
          man = "/out/share/man/man1/rg.1",
        }
      end,
    })

    -- Build fd (another CLI tool)
    -- Demonstrates simpler build with environment variables
    local fd = sys.build({
      name = "fd",
      version = "9.0.0",
      apply = function(build_inputs, ctx)
        ctx:cmd({
          cmd = "echo 'Building fd with HOME=$HOME'",
          env = { CARGO_HOME = "$HOME/.cargo" }, -- Shell vars in env values
        })
        return { out = "/store/fd" }
      end,
    })

    -- Bind ripgrep to system
    -- Demonstrates using build outputs in bind commands
    sys.bind({
      inputs = { rg = rg },
      apply = function(bind_inputs, ctx)
        -- Reference build output via inputs
        -- Shell variable $HOME works naturally
        ctx:cmd({
          cmd = "ln -sf " .. bind_inputs.rg.outputs.bin .. " $HOME/.local/bin/rg",
        })

        -- Create shell completion using multiple build outputs
        ctx:cmd({
          cmd = "mkdir -p $HOME/.local/share/man/man1 && "
            .. "ln -sf "
            .. bind_inputs.rg.outputs.man
            .. " $HOME/.local/share/man/man1/rg.1",
        })
      end,
      destroy = function(bind_inputs, ctx)
        -- Cleanup commands - shell variables work naturally
        ctx:cmd({ cmd = "rm -f $HOME/.local/bin/rg" })
        ctx:cmd({ cmd = "rm -f $HOME/.local/share/man/man1/rg.1" })
      end,
    })

    -- Bind fd to system
    sys.bind({
      inputs = { fd = fd },
      apply = function(bind_inputs, ctx)
        ctx:cmd({
          cmd = "ln -sf " .. bind_inputs.fd.outputs.out .. "/bin/fd $HOME/.local/bin/fd",
        })
      end,
      destroy = function(bind_inputs, ctx)
        ctx:cmd({ cmd = "rm -f $HOME/.local/bin/fd" })
      end,
    })

    -- Bind that creates an env file combining multiple builds
    -- Demonstrates referencing multiple builds and complex shell variable usage
    sys.bind({
      inputs = { rg = rg, fd = fd },
      apply = function(bind_inputs, ctx)
        -- Create an env.sh that sets up PATH with both tools
        -- Note: $PATH at end is a shell variable (preserved)
        -- The build output paths are Lua string concatenation
        local env_content = "export PATH="
          .. bind_inputs.rg.outputs.out
          .. "/bin:"
          .. bind_inputs.fd.outputs.out
          .. "/bin:$PATH"

        ctx:cmd({
          cmd = 'echo "' .. env_content .. '" > $HOME/.local/env.sh',
        })
      end,
      destroy = function(bind_inputs, ctx)
        ctx:cmd({ cmd = "rm -f $HOME/.local/env.sh" })
      end,
    })
  end,
}
