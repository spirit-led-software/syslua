local prio = require('syslua.priority')
local lib = require('syslua.lib')

---@class syslua.pkgs.cli.ripgrep
local M = {}

---@type syslua.pkgs.Releases
M.releases = {
  ['15.1.0'] = {
    ['aarch64-darwin'] = {
      url = 'https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-aarch64-apple-darwin.tar.gz',
      sha256 = '378e973289176ca0c6054054ee7f631a065874a352bf43f0fa60ef079b6ba715',
      format = 'tar.gz',
    },
    ['x86_64-darwin'] = {
      url = 'https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-x86_64-apple-darwin.tar.gz',
      sha256 = '64811cb24e77cac3057d6c40b63ac9becf9082eedd54ca411b475b755d334882',
      format = 'tar.gz',
    },
    ['x86_64-linux'] = {
      url = 'https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz',
      sha256 = '1c9297be4a084eea7ecaedf93eb03d058d6faae29bbc57ecdaf5063921491599',
      format = 'tar.gz',
    },
    ['x86_64-windows'] = {
      url = 'https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-x86_64-pc-windows-msvc.zip',
      sha256 = '124510b94b6baa3380d051fdf4650eaa80a302c876d611e9dba0b2e18d87493a',
      format = 'zip',
    },
  },
}

---@type syslua.pkgs.Meta
M.meta = {
  name = 'ripgrep',
  homepage = 'https://github.com/BurntSushi/ripgrep',
  description = 'ripgrep recursively searches directories for a regex pattern',
  license = 'MIT',
  versions = {
    stable = '15.1.0',
    latest = '15.1.0',
  },
}

-- ============================================================================
-- Options
-- ============================================================================

---@class syslua.pkgs.cli.ripgrep.Options
---@field version? string | syslua.priority.PriorityValue<string>

local default_opts = {
  version = prio.default(M.meta.versions.stable),
}

---@type syslua.pkgs.cli.ripgrep.Options
M.opts = default_opts

-- ============================================================================
-- Setup
-- ============================================================================

---Build ripgrep package
---@param provided_opts? syslua.pkgs.cli.ripgrep.Options
---@return BuildRef
function M.setup(provided_opts)
  local new_opts = prio.merge(M.opts, provided_opts or {})
  if not new_opts then
    error('Failed to merge ripgrep options')
  end
  M.opts = new_opts

  -- Resolve version alias
  local version = M.meta.versions[M.opts.version] or M.opts.version

  local release = M.releases[version]
  if not release then
    local available = {}
    for v in pairs(M.releases) do
      table.insert(available, v)
    end
    table.sort(available)
    error(string.format("ripgrep version '%s' not found. Available: %s", version, table.concat(available, ', ')))
  end

  local platform_release = release[sys.platform]
  if not platform_release then
    local available = {}
    for p in pairs(release) do
      table.insert(available, p)
    end
    table.sort(available)
    error(
      string.format(
        'ripgrep %s not available for %s. Available: %s',
        version,
        sys.platform,
        table.concat(available, ', ')
      )
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
      local bin_name = 'rg' .. (sys.os == 'windows' and '.exe' or '')
      local man_name = 'doc/rg.1'
      local completions_dir = 'complete'

      local src = inputs.extracted.outputs.out
      if sys.os == 'windows' then
        ctx:exec({
          bin = 'cmd.exe',
          args = {
            '/c',
            string.format(
              'copy "%s\\%s" "%s\\" && mkdir "%s\\doc" && copy "%s\\%s" "%s\\doc\\" && xcopy /E /I "%s\\%s" "%s\\%s"',
              src,
              bin_name,
              ctx.out,
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
        ctx:exec({ bin = '/bin/mkdir', args = { '-p', ctx.out .. '/doc' } })
        ctx:exec({ bin = '/bin/cp', args = { src .. '/' .. man_name, ctx.out .. '/doc/' } })
        ctx:exec({ bin = '/bin/cp', args = { '-r', src .. '/' .. completions_dir, ctx.out .. '/' } })
      end
      return {
        bin = sys.path.join(ctx.out, bin_name),
        man = sys.path.join(ctx.out, 'doc', 'rg.1'),
        completions = sys.path.join(ctx.out, completions_dir),
        out = ctx.out,
      }
    end,
  })
end

return M
