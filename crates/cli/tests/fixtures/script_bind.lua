return {
  inputs = {
    syslua = 'path:./syslua',
  },
  setup = function()
    require('syslua').setup()

    local marker_path = sys.getenv('TEST_OUTPUT_DIR') .. '/bind-script-marker.txt'

    sys.bind({
      id = 'test-bind-script',
      create = function(_inputs, ctx)
        ctx:script('shell', 'touch "' .. marker_path .. '"')
        return { out = ctx.out }
      end,
      destroy = function(outputs, ctx)
        ctx:script('shell', 'rm -f "' .. marker_path .. '"')
      end,
    })
  end,
}
