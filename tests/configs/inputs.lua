--- Input resolution test configuration
--- Tests that git and path inputs are resolved correctly, including #ref syntax

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
  inputs = {
    -- Git input from GitHub with specific commit ref
    -- Using the initial commit of the repo for a stable test
    syslua = 'git:https://github.com/spirit-led-software/syslua.git#3d522f5e2baf56a5e2f750d4664c174a2099833b',
  },
  setup = function(inputs)
    -- Verify the syslua input was resolved
    assert(inputs.syslua, 'syslua input should be present')
    assert(inputs.syslua.path, 'syslua input should have a path')
    assert(inputs.syslua.rev, 'syslua input should have a rev')
    assert(#inputs.syslua.rev == 40, 'syslua rev should be a full git hash (40 chars)')

    -- Verify the rev matches what we requested (it should resolve to the same commit)
    assert(inputs.syslua.rev == '3d522f5e2baf56a5e2f750d4664c174a2099833b', 'syslua rev should match requested commit')

    -- Print input info for debugging
    print('syslua input resolved:')
    print('  path: ' .. inputs.syslua.path)
    print('  rev:  ' .. inputs.syslua.rev)

    -- Create a simple build that uses the input
    local example = sys.build({
      name = 'example-from-input',
      version = '1.0.0',
      inputs = {
        src = inputs.syslua,
      },
      apply = function(build_inputs, ctx)
        -- Reference the input path in a build command
        sh(ctx, 'ls -la ' .. build_inputs.src.path)
        return { out = '/store/example' }
      end,
    })

    -- Bind that references the build
    sys.bind({
      inputs = { example = example },
      apply = function(bind_inputs, ctx)
        sh(ctx, "echo 'Example output: " .. bind_inputs.example.outputs.out .. "'")
      end,
    })
  end,
}
