--- Basic test configuration
--- Entry point returns a table with `inputs` and `setup` fields

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
    local rg = sys.build({
      name = 'ripgrep',
      version = '15.0.0',
      apply = function(_, ctx)
        sh(ctx, "echo 'building ripgrep'")
        return { out = ctx.out }
      end,
    })

    sys.bind({
      inputs = { build = rg },
      apply = function(bind_inputs, ctx)
        sh(
          ctx,
          'mkdir -p /tmp/syslua-test && ln -sf '
            .. bind_inputs.build.outputs.out
            .. '/bin/rg /tmp/syslua-test/rg'
        )
        return { link = '/tmp/syslua-test/rg' }
      end,
      destroy = function(outputs, ctx)
        sh(ctx, 'rm -f ' .. outputs.link)
      end,
    })
  end,
}
