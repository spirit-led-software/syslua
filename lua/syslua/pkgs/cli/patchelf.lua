local prio = require('syslua.priority')
local lib = require('syslua.lib')

---@class syslua.pkgs.cli.patchelf
local M = {}

---@type syslua.pkgs.Releases
M.releases = {
  ['0.18.0'] = {
    ['x86_64-linux'] = {
      url = 'https://github.com/NixOS/patchelf/releases/download/0.18.0/patchelf-0.18.0-x86_64.tar.gz',
      sha256 = 'ce84f2447fb7a8679e58bc54a20dc2b01b37b5802e12c57eece772a6f14bf3f0',
      format = 'tar.gz',
    },
    ['aarch64-linux'] = {
      url = 'https://github.com/NixOS/patchelf/releases/download/0.18.0/patchelf-0.18.0-aarch64.tar.gz',
      sha256 = 'ae13e2effe077e829be759182396b931d8f85cfb9cfe9d49385516ea367ef7b2',
      format = 'tar.gz',
    },
    ['x86_64-windows'] = {
      url = 'https://github.com/NixOS/patchelf/releases/download/0.18.0/patchelf-win64-0.18.0.exe',
      sha256 = 'ea5293833b6a547612ce4b073ac84fd603011ce3455f488a1017fabc8bd170ff',
      format = 'binary',
    },
  },
}

---@type syslua.pkgs.Meta
M.meta = {
  name = 'patchelf',
  homepage = 'https://github.com/NixOS/patchelf',
  description = 'A utility for modifying existing ELF executables and libraries',
  license = 'GPL-3.0',
  versions = {
    stable = '0.18.0',
    latest = '0.18.0',
  },
}

---@class syslua.pkgs.cli.patchelf.Options
---@field version? string | syslua.priority.PriorityValue<string>

local default_opts = {
  version = prio.default(M.meta.versions.stable),
}

---@type syslua.pkgs.cli.patchelf.Options
M.opts = default_opts

---@param provided_opts? syslua.pkgs.cli.patchelf.Options
---@return BuildRef
function M.setup(provided_opts)
  local new_opts = prio.merge(M.opts, provided_opts or {})
  if not new_opts then
    error('Failed to merge patchelf options')
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
    error(string.format("patchelf version '%s' not found. Available: %s", version, table.concat(available, ', ')))
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
        'patchelf %s not available for %s. Available: %s',
        version,
        sys.platform,
        table.concat(available, ', ')
      )
    )
  end

  local downloaded = lib.fetch_url({
    url = platform_release.url,
    sha256 = platform_release.sha256,
  })

  if platform_release.format == 'binary' then
    return sys.build({
      inputs = {
        downloaded = downloaded,
      },
      create = function(inputs, ctx)
        local src = inputs.downloaded.outputs.out
        local bin_name = 'patchelf.exe'
        local bin_path = sys.path.join(ctx.out, bin_name)

        ctx:exec({
          bin = 'cmd.exe',
          args = { '/c', string.format('copy "%s" "%s"', src, bin_path) },
        })

        return {
          bin = bin_path,
          out = ctx.out,
        }
      end,
    })
  end

  local extracted = lib.extract({
    archive = downloaded.outputs.out,
    format = platform_release.format,
    strip_components = 0,
  })

  return sys.build({
    inputs = {
      extracted = extracted,
    },
    create = function(inputs, ctx)
      local bin_name = 'patchelf'
      local src = inputs.extracted.outputs.out

      ctx:exec({ bin = '/bin/cp', args = { src .. '/bin/' .. bin_name, ctx.out .. '/' } })
      ctx:exec({ bin = '/bin/chmod', args = { '+x', ctx.out .. '/' .. bin_name } })

      return {
        bin = sys.path.join(ctx.out, bin_name),
        out = ctx.out,
      }
    end,
  })
end

return M
