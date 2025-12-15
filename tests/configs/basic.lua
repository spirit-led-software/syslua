--- Basic test configuration
--- Entry point returns a table with `inputs` and `setup` fields
return {
  inputs = {},
  setup = function(inputs)
    local rg = sys.build({
      name = "ripgrep",
      version = "15.0.0",
      apply = function(build_inputs, ctx)
        ctx:cmd({ cmd = "echo 'building ripgrep'" })
        return { out = "/store/ripgrep" }
      end,
    })

    sys.bind({
      inputs = { build = rg },
      apply = function(bind_inputs, ctx)
        ctx:cmd({ cmd = "ln -sf " .. bind_inputs.build.outputs.out .. "/bin/rg /usr/local/bin/rg" })
      end,
      destroy = function(bind_inputs, ctx)
        ctx:cmd({ cmd = "rm /usr/local/bin/rg" })
      end,
    })
  end,
}
