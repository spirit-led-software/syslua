return {
  inputs = {
    syslua = 'path:./syslua',
  },
  setup = function()
    require('syslua').setup()

    sys.build({
      id = 'test-script-cmd',
      create = function(_inputs, ctx)
        local result = ctx:script(
          'cmd',
          [[
@echo off
echo hello from cmd
]]
        )
        return { out = ctx.out }
      end,
    })
  end,
}
