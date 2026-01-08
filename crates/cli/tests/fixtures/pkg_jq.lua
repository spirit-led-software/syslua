return {
  inputs = {
    syslua = 'path:./lua',
  },
  setup = function()
    local syslua = require('syslua')

    local ref = syslua.pkgs.cli.jq.setup()

    sys.bind({
      inputs = { jq = ref },
      create = function(inputs, ctx)
        local bin_path = inputs.jq.outputs.bin
        if sys.os == 'windows' then
          ctx:exec({ bin = 'cmd.exe', args = { '/c', 'if exist "' .. bin_path .. '" echo exists' } })
        else
          ctx:exec({ bin = '/bin/sh', args = { '-c', 'test -x "' .. bin_path .. '" && echo executable' } })
        end
        return { bin_path = bin_path }
      end,
      destroy = function(outputs, ctx) end,
    })
  end,
}
