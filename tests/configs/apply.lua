--- Apply flow test configuration
--- Tests the full apply workflow including:
--- - Build realization and caching
--- - Bind apply and destroy actions
--- - State tracking via snapshots
--- - Diff computation between states
---
--- Usage:
---   # First apply - creates files
---   sys apply tests/configs/apply.lua
---
---   # Second apply - should show cached builds, unchanged binds
---   sys apply tests/configs/apply.lua
---
---   # Modify this file (e.g., add/remove a bind) and re-apply to test diff
return {
  inputs = {},
  setup = function()
    -- Build 1: Simple text file generator
    -- Creates a marker file in the store to verify build execution
    local greeter = sys.build({
      name = 'greeter',
      version = '1.0.0',
      apply = function(_, ctx)
        -- ctx.out returns the $${out} placeholder
        ctx:cmd({ cmd = 'mkdir -p ' .. ctx.out .. '/bin' })
        ctx:cmd({
          cmd = string.format(
            [[
              echo -e '#!/bin/sh\necho "Hello from greeter!"' > %s/bin/greet'
            ]],
            ctx.out
          ),
        })
        ctx:cmd({ cmd = 'chmod +x ' .. ctx.out .. '/bin/greet' })

        return {
          out = ctx.out,
          bin = ctx.out .. '/bin/greet',
        }
      end,
    })

    -- Build 2: Another simple build to test parallel execution and caching
    local counter = sys.build({
      name = 'counter',
      version = '1.0.0',
      apply = function(_, ctx)
        ctx:cmd({ cmd = 'mkdir -p ' .. ctx.out .. '/bin' })
        ctx:cmd({
          cmd = "echo '#!/bin/sh\nseq 1 10' > " .. ctx.out .. '/bin/count',
        })
        ctx:cmd({ cmd = 'chmod +x ' .. ctx.out .. '/bin/count' })

        return {
          out = ctx.out,
          bin = ctx.out .. '/bin/count',
        }
      end,
    })

    -- Build 3: Dependent build - depends on greeter
    -- Tests DAG ordering and input resolution
    local wrapper = sys.build({
      name = 'wrapper',
      version = '1.0.0',
      inputs = { greeter = greeter },
      apply = function(build_inputs, ctx)
        ctx:cmd({ cmd = 'mkdir -p ' .. ctx.out .. '/bin' })
        -- Create a wrapper script that calls greeter
        ctx:cmd({
          cmd = "echo '#!/bin/sh\n"
            .. build_inputs.greeter.outputs.bin
            .. ' && echo "Wrapper done!"\' > '
            .. ctx.out
            .. '/bin/wrap',
        })
        ctx:cmd({ cmd = 'chmod +x ' .. ctx.out .. '/bin/wrap' })

        return {
          out = ctx.out,
          bin = ctx.out .. '/bin/wrap',
        }
      end,
    })

    -- Bind 1: Link greeter to a temp location
    -- Tests basic bind with destroy action
    sys.bind({
      inputs = { greeter = greeter },
      apply = function(bind_inputs, ctx)
        ctx:cmd({ cmd = 'mkdir -p /tmp/syslua-test' })
        ctx:cmd({
          cmd = 'ln -sf ' .. bind_inputs.greeter.outputs.bin .. ' /tmp/syslua-test/greet',
        })
        return { link = '/tmp/syslua-test/greet' }
      end,
      destroy = function(_, ctx)
        ctx:cmd({ cmd = 'rm -f /tmp/syslua-test/greet' })
      end,
    })

    -- Bind 2: Link counter to a temp location
    -- Tests multiple independent binds
    sys.bind({
      inputs = { counter = counter },
      apply = function(bind_inputs, ctx)
        ctx:cmd({ cmd = 'mkdir -p /tmp/syslua-test' })
        ctx:cmd({
          cmd = 'ln -sf ' .. bind_inputs.counter.outputs.bin .. ' /tmp/syslua-test/count',
        })
        return { link = '/tmp/syslua-test/count' }
      end,
      destroy = function(outputs, ctx)
        ctx:cmd({ cmd = 'rm -f ' .. outputs.link })
      end,
    })

    -- Bind 3: Link wrapper (depends on greeter build via wrapper build)
    -- Tests bind with transitive build dependencies
    sys.bind({
      inputs = { wrapper = wrapper },
      apply = function(bind_inputs, ctx)
        ctx:cmd({ cmd = 'mkdir -p /tmp/syslua-test' })
        ctx:cmd({
          cmd = 'ln -sf ' .. bind_inputs.wrapper.outputs.bin .. ' /tmp/syslua-test/wrap',
        })
        return { link = '/tmp/syslua-test/wrap' }
      end,
      destroy = function(outputs, ctx)
        ctx:cmd({ cmd = 'rm -f ' .. outputs.link })
      end,
    })

    -- Bind 4: Create a marker file (no build dependency)
    -- Tests bind-only execution
    sys.bind({
      outputs = { marker = '/tmp/syslua-test/marker.txt' },
      apply = function(bind_inputs, ctx)
        ctx:cmd({ cmd = 'mkdir -p /tmp/syslua-test' })
        ctx:cmd({
          cmd = 'echo "Applied at $(date)" > /tmp/syslua-test/marker.txt',
        })
      end,
      destroy = function(bind_inputs, ctx)
        ctx:cmd({ cmd = 'rm -f /tmp/syslua-test/marker.txt' })
      end,
    })

    -- Bind 5: Env file combining multiple builds
    -- Tests bind with multiple build inputs
    sys.bind({
      inputs = { greeter = greeter, counter = counter },
      apply = function(bind_inputs, ctx)
        ctx:cmd({ cmd = 'mkdir -p /tmp/syslua-test' })
        local content = '# syslua test environment\\n'
          .. 'export GREETER_BIN='
          .. bind_inputs.greeter.outputs.bin
          .. '\\n'
          .. 'export COUNTER_BIN='
          .. bind_inputs.counter.outputs.bin
          .. '\\n'
        ctx:cmd({
          cmd = 'printf "' .. content .. '" > /tmp/syslua-test/env.sh',
        })
        return { env = '/tmp/syslua-test/env.sh' }
      end,
      destroy = function(outputs, ctx)
        ctx:cmd({ cmd = 'rm -f ' .. outputs.env })
      end,
    })
  end,
}
