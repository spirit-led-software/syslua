local prio = require('syslua.priority')
local lib = require('syslua.lib')

---@class syslua.pkgs.cli.fd
local M = {}

---@type syslua.pkgs.Releases
M.releases = {
  ['v10.2.0'] = {
    ['aarch64-darwin'] = {
      url = 'https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-aarch64-apple-darwin.tar.gz',
      sha256 = 'ae6327ba8c9a487cd63edd8bddd97da0207887a66d61e067dfe80c1430c5ae36',
      format = 'tar.gz',
    },
    ['x86_64-darwin'] = {
      url = 'https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-x86_64-apple-darwin.tar.gz',
      sha256 = '991a648a58870230af9547c1ae33e72cb5c5199a622fe5e540e162d6dba82d48',
      format = 'tar.gz',
    },
    ['x86_64-linux'] = {
      url = 'https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-x86_64-unknown-linux-musl.tar.gz',
      sha256 = 'd9bfa25ec28624545c222992e1b00673b7c9ca5eb15393c40369f10b28f9c932',
      format = 'tar.gz',
    },
    ['x86_64-windows'] = {
      url = 'https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-x86_64-pc-windows-msvc.zip',
      sha256 = '92ac9e6b0a0c6ecdab638ffe210dc786403fff4c66373604cf70df27be45e4fe',
      format = 'zip',
    },
  },
}

---@type syslua.pkgs.Meta
M.meta = {
  name = 'fd',
  homepage = 'https://github.com/sharkdp/fd',
  description = 'A simple, fast and user-friendly alternative to find',
  license = 'MIT',
  versions = {
    stable = 'v10.2.0',
    latest = 'v10.2.0',
  },
}

---@class syslua.pkgs.cli.fd.Options
---@field version? string | syslua.priority.PriorityValue<string>

local default_opts = {
  version = prio.default(M.meta.versions.stable),
}

---@type syslua.pkgs.cli.fd.Options
M.opts = default_opts

---@param provided_opts? syslua.pkgs.cli.fd.Options
---@return BuildRef
function M.setup(provided_opts)
  local new_opts = prio.merge(M.opts, provided_opts or {})
  if not new_opts then
    error('Failed to merge fd options')
  end
  M.opts = new_opts

  local version = M.meta.versions[M.opts.version] or M.opts.version

  local release = M.releases[version]
  if not release then
    local available = {}
    for v in pairs(M.releases) do
      table.insert(available, v)
    end
    table.sort(available)
    error(string.format("fd version '%s' not found. Available: %s", version, table.concat(available, ', ')))
  end

  local platform_release = release[sys.platform]
  if not platform_release then
    local available = {}
    for p in pairs(release) do
      table.insert(available, p)
    end
    table.sort(available)
    error(
      string.format('fd %s not available for %s. Available: %s', version, sys.platform, table.concat(available, ', '))
    )
  end

  local archive = lib.fetch_url({
    url = platform_release.url,
    sha256 = platform_release.sha256,
  })

  local extracted = lib.extract({
    archive = archive.outputs.out,
    format = platform_release.format,
    strip_components = 1,
  })

  return sys.build({
    inputs = {
      extracted = extracted,
    },
    create = function(inputs, ctx)
      local bin_name = 'fd' .. (sys.os == 'windows' and '.exe' or '')
      local man_name = 'fd.1'
      local completions_dir = 'autocomplete'

      local src = inputs.extracted.outputs.out
      if sys.os == 'windows' then
        ctx:exec({
          bin = 'cmd.exe',
          args = {
            '/c',
            string.format(
              'copy "%s\\%s" "%s\\" && copy "%s\\%s" "%s\\" && xcopy /E /I "%s\\%s" "%s\\%s"',
              src,
              bin_name,
              ctx.out,
              src,
              man_name,
              ctx.out,
              src,
              completions_dir,
              ctx.out,
              completions_dir
            ),
          },
        })
      else
        ctx:exec({ bin = '/bin/cp', args = { src .. '/' .. bin_name, ctx.out .. '/' } })
        ctx:exec({ bin = '/bin/cp', args = { src .. '/' .. man_name, ctx.out .. '/' } })
        ctx:exec({ bin = '/bin/cp', args = { '-r', src .. '/' .. completions_dir, ctx.out .. '/' } })
      end
      return {
        bin = sys.path.join(ctx.out, bin_name),
        man = sys.path.join(ctx.out, man_name),
        completions = sys.path.join(ctx.out, completions_dir),
        out = ctx.out,
      }
    end,
  })
end

return M
