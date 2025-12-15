---@class syslua.modules.file
local M = {}

---@class FileOptions
---@field target string Path to the target file or directory
---@field source? string Path to the source file or directory
---@field content? string Content to write to the target file (if source is not provided)
---@field mutable? boolean Whether the target should be mutable (default: false)

local default_options = {
  mutable = false,
}

--- Set up a file or directory according to the provided options
---@param opts FileOptions
M.setup = function(opts)
  if not opts.target then
    error("File setup requires a 'target' option")
  end

  if not opts.source and not opts.content then
    error("File setup requires either a 'source' or 'content' option")
  end

  local mutable = opts.mutable or default_options.mutable

  if mutable then
    sys.bind({
      inputs = {
        target = opts.target,
        source = opts.source,
        content = opts.content,
        mutable = mutable,
      },
      apply = function(inputs, ctx)
        if opts.source then
          if sys.os == 'windows' then
            ctx:cmd({
              cmd = string.format('xcopy /E /I /Y "%s" "%s"', inputs.source, inputs.target),
            })
          else
            ctx:cmd({
              cmd = string.format('cp -r "%s" "%s"', inputs.source, inputs.target),
            })
          end
        else
          ctx:cmd({
            cmd = string.format('echo "%s" > "%s"', inputs.content, inputs.target),
          })
        end
      end,
    })
  else
    local basename = sys.path.basename(opts.target)
    local build = sys.build({
      name = basename .. '_bld',
      inputs = {
        source = opts.source,
        content = opts.content,
        mutable = mutable,
      },
      apply = function(inputs, ctx)
        if inputs.source then
          if sys.os == 'windows' then
            ctx:cmd({
              cmd = string.format('xcopy /E /I /Y "%s" "%s"', inputs.source, basename),
            })
          else
            ctx:cmd({
              cmd = string.format('cp -r "%s" "%s"', inputs.source, basename),
            })
          end
        else
          ctx:cmd({
            cmd = string.format('echo "%s" > "%s"', inputs.content, basename),
          })
        end

        return {
          out = basename,
        }
      end,
    })

    sys.bind({
      inputs = {
        build = build,
        target = opts.target,
      },
      apply = function(inputs, ctx)
        if sys.os == 'windows' then
          ctx:cmd({
            cmd = string.format(
              'New-Item -ItemType SymbolicLink -Path "%s" -Target "%s"',
              inputs.target,
              inputs.build.outputs.out
            ),
          })
        else
          ctx:cmd({
            cmd = string.format('ln -s "%s" "%s"', inputs.build.outputs.out, inputs.target),
          })
        end
      end,
      destroy = function(_, ctx)
        if sys.os == 'windows' then
          ctx:cmd({
            cmd = string.format('Remove-Item -Path "%s" -Recurse -Force', opts.target),
          })
        else
          ctx:cmd({
            cmd = string.format('rm -rf "%s"', opts.target),
          })
        end
      end,
    })
  end
end

return M
