# Inputs and Registry

> Part of the [sys.lua Architecture](./00-overview.md) documentation.

This document covers input sources, registry structure, lock files, and authentication.

## Overview

Instead of a separate registry sync mechanism, sys.lua uses declarative inputs defined in the config file itself, similar to Nix Flakes.

## Input Declaration

```lua
-- sys.lua
local lib = require("syslua.lib")
local secrets = sops.load("./secrets.yaml")  -- For private input auth

local inputs = {
    -- Official package registry (public, no auth)
    pkgs = input "github:sys-lua/pkgs" {
        rev = "a1b2c3d4...",  -- pinned commit (optional, defaults to latest)
    },

    -- Additional package sets
    unstable = input "github:sys-lua/pkgs" {
        branch = "unstable",
    },

    -- Private/corporate registry (authenticated via SOPS)
    company = input "github:mycompany/sys-pkgs" {
        rev = "...",
        auth = secrets.github_token,  -- GitHub PAT from secrets
    },

    -- Local path (for development)
    local_pkgs = input "path:./my-packages",

    -- Git URL
    custom = input "git:https://git.example.com/pkgs.git" {
        rev = "v1.0.0",
    },
}

-- Use packages from inputs (latest version in registry)
require("inputs.pkgs.cli.ripgrep").setup()
require("inputs.unstable.cli.neovim").setup()
require("inputs.company.internal_tool").setup()

-- Pin to specific version
require("inputs.pkgs.cli.ripgrep").setup({ version = "14_1_0" })
```

## Registry Structure

The official registry uses a hierarchical structure with `init.lua` entry points and versioned implementation files. Version files use underscores (e.g., `15_1_0.lua`) since Lua `require` doesn't work well with dots in module names.

```
sys-lua/pkgs/
├── cli/
│   ├── init.lua              # Category entry point
│   ├── ripgrep/
│   │   ├── init.lua          # Package entry point (latest + version routing)
│   │   ├── 15_1_0.lua        # Version implementation
│   │   ├── 14_1_0.lua
│   │   └── 13_0_0.lua
│   └── fd/
│       ├── init.lua
│       ├── 9_0_0.lua
│       └── 8_7_0.lua
├── editors/
│   ├── init.lua
│   ├── neovim/
│   │   ├── init.lua
│   │   ├── 0_10_0.lua
│   │   └── 0_9_5.lua
│   └── helix/
│       └── ...
└── init.lua                  # Root entry point
```

### Version File Example

**`pkgs/cli/ripgrep/15_1_0.lua`:**

```lua
---@class pkgs.cli.ripgrep.15_1_0
local M = {}

local hashes = {
    ["aarch64-darwin"] = "abc123...",
    ["x86_64-linux"] = "def456...",
    ["x86_64-windows"] = "ghi789...",
}

M.make_derivation = function()
    return derive({
        name = "ripgrep",
        version = "15.1.0",
        opts = function(sys)
            return {
                url = "https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-" .. sys.platform .. ".tar.gz",
                sha256 = hashes[sys.platform],
            }
        end,
        config = function(opts, ctx)
            local archive = ctx.fetch_url(opts.url, opts.sha256)
            ctx.unpack(archive, ctx.out)
        end,
    })
end

M.make_activation = function(drv)
    return activate({
        opts = { drv = drv },
        config = function(opts, ctx)
            ctx.add_to_path(opts.drv.out .. "/bin")
        end,
    })
end

M.setup = function()
    local derivation = M.make_derivation()
    M.make_activation(derivation)
end

return M
```

### Package Entry Point Example

**`pkgs/cli/ripgrep/init.lua`:**

```lua
---@class pkgs.cli.ripgrep
---@field ["15_1_0"] pkgs.cli.ripgrep.15_1_0
---@field ["14_1_0"] pkgs.cli.ripgrep.14_1_0
local M = {}

-- Lazy loading of version modules
setmetatable(M, {
    __index = function(_, pkg)
        return require("pkgs.cli.ripgrep." .. pkg)
    end,
})

M.setup = function(opts)
    if opts == nil then
        return require("pkgs.cli.ripgrep.15_1_0").setup()
    end

    local version = opts.version or "15_1_0"
    local version_module = require("pkgs.cli.ripgrep." .. version)

    if opts.make_derivation then
        version_module.make_derivation = opts.make_derivation
    end
    if opts.make_activation then
        version_module.make_activation = opts.make_activation
    end

    return version_module.setup()
end

return M
```

### Version Selection

| Usage                                                              | Behavior              |
| ------------------------------------------------------------------ | --------------------- |
| `require("inputs.pkgs.cli.ripgrep").setup()`                       | Uses latest (15_1_0)  |
| `require("inputs.pkgs.cli.ripgrep").setup({ version = "14_1_0" })` | Uses specific version |
| `require("inputs.pkgs.cli.ripgrep")["14_1_0"].setup()`             | Direct version access |

## Package References

When you access `inputs.pkgs.cli.ripgrep`, it returns a **package module** with factory functions and a `setup()` method:

```lua
-- What inputs.pkgs.cli.ripgrep resolves to:
{
    -- Factory functions
    make_derivation = function() ... end,
    make_activation = function(drv) ... end,

    -- Setup orchestrates the installation
    setup = function(opts) ... end,

    -- Version modules accessible via metatable
    ["15_1_0"] = <lazy loaded version module>,
    ["14_1_0"] = <lazy loaded version module>,
}

-- Usage:
require("inputs.pkgs.cli.ripgrep").setup()                         -- Latest version
require("inputs.pkgs.cli.ripgrep").setup({ version = "14_1_0" })   -- Specific version
require("inputs.pkgs.cli.ripgrep")["14_1_0"].setup()               -- Direct access
```

**Note:** There is no separate `pkg()` function. Packages are installed by calling `setup()` on the package module, which internally calls `derive()` and `activate()` to register the package.

## Lock File

sys.lua generates a `syslua.lock` file in the same directory as the configuration. This enables:

- **System configs**: `/etc/syslua/` → `/etc/syslua/syslua.lock`
- **User configs**: `~/.config/syslua/` → `~/.config/syslua/syslua.lock`
- **Project configs**: `./` → `./syslua.lock` (committed to version control)

### Lock File Format

```json
{
  "version": 1,
  "inputs": {
    "pkgs": {
      "type": "github",
      "owner": "sys-lua",
      "repo": "pkgs",
      "rev": "a1b2c3d4e5f6...",
      "sha256": "...",
      "lastModified": 1733667300
    },
    "unstable": {
      "type": "github",
      "owner": "sys-lua",
      "repo": "pkgs",
      "branch": "unstable",
      "rev": "f6e5d4c3b2a1...",
      "sha256": "...",
      "lastModified": 1733667400
    }
  }
}
```

### Lock File Behavior

| Scenario              | Behavior                                 |
| --------------------- | ---------------------------------------- |
| `syslua.lock` exists  | Use pinned revisions from lock file      |
| `syslua.lock` missing | Resolve latest, create lock file         |
| `sys update`          | Re-resolve specified inputs, update lock |
| `sys update --commit` | Update lock and `git commit` it          |

### Team Workflow

```bash
# Developer A: Add new input, commit lock file
git add init.lua syslua.lock
git commit -m "Add nodejs to project"

# Developer B: Pull and apply (uses same pinned versions)
git pull
sudo sys apply sys.lua
```

### Commands

```bash
sys update                    # Update all inputs to latest
sys update pkgs               # Update specific input
sys update --commit           # Update and commit lock file
sys update --dry-run          # Show what would change
```

## Input Authentication

Private inputs (corporate registries, private GitHub repos) require authentication. sys.lua uses **SOPS-encrypted secrets** for secure credential storage:

```yaml
# secrets.yaml (encrypted with SOPS)
github_token: ENC[AES256_GCM,data:...,tag:...]
gitlab_token: ENC[AES256_GCM,data:...,tag:...]
```

```lua
-- sys.lua
local secrets = sops.load("./secrets.yaml")

local inputs = {
    -- Public input (no auth)
    pkgs = input "github:sys-lua/pkgs",

    -- Private GitHub input
    company = input "github:mycompany/private-pkgs" {
        auth = secrets.github_token,
    },

    -- Private GitLab input
    internal = input "gitlab:internal.company.com/pkgs" {
        auth = secrets.gitlab_token,
    },

    -- SSH-based input (uses system SSH keys)
    private = input "git:git@github.com:mycompany/pkgs.git" {
        -- No auth needed, uses ~/.ssh/id_* keys
    },
}
```

### Authentication Methods

| Input Type          | Auth Method                  |
| ------------------- | ---------------------------- |
| `github:` (public)  | None required                |
| `github:` (private) | `auth = "<PAT>"` from SOPS   |
| `gitlab:`           | `auth = "<token>"` from SOPS |
| `git:` (HTTPS)      | `auth = "<token>"` from SOPS |
| `git:` (SSH)        | System SSH keys (~/.ssh/)    |

### Security Notes

- Never commit plaintext tokens to `sys.lua`
- Use SOPS to encrypt credentials in `secrets.yaml`
- The `auth` field is never written to `syslua.lock`

## Input Resolution Algorithm

```
RESOLVE_INPUTS(config, lock_file):
    inputs = {}

    FOR EACH input_decl IN config.inputs:
        input_id = input_decl.name

        // Check if lock file exists and has this input
        IF lock_file EXISTS AND lock_file.inputs[input_id] EXISTS:
            locked = lock_file.inputs[input_id]

            // Validate lock entry matches config
            IF locked.type != input_decl.type OR locked.url != input_decl.url:
                ERROR "Lock file mismatch for input '{input_id}'."
                      "Run 'sys update {input_id}' to update the lock file."

            // Use pinned revision from lock
            inputs[input_id] = FETCH_INPUT(input_decl, locked.rev)
        ELSE:
            // No lock entry - resolve to latest
            resolved = RESOLVE_LATEST(input_decl)
            inputs[input_id] = FETCH_INPUT(input_decl, resolved.rev)

            // Add to lock file
            lock_file.inputs[input_id] = {
                type: input_decl.type,
                url: input_decl.url,
                rev: resolved.rev,
                sha256: resolved.sha256,
                lastModified: resolved.timestamp,
            }

    // Write updated lock file if changed
    IF lock_file WAS MODIFIED:
        WRITE_LOCK_FILE(lock_file)

    RETURN inputs

RESOLVE_LATEST(input_decl):
    SWITCH input_decl.type:
        CASE "github":
            IF input_decl.branch SPECIFIED:
                RETURN GITHUB_API.get_branch_head(owner, repo, branch)
            ELSE:
                RETURN GITHUB_API.get_default_branch_head(owner, repo)

        CASE "gitlab":
            // Similar to GitHub

        CASE "git":
            RETURN GIT.ls_remote(url, ref="HEAD")

        CASE "path":
            // Local paths use directory mtime as "revision"
            RETURN { rev: "local", sha256: HASH_DIRECTORY(path), timestamp: DIR_MTIME(path) }

FETCH_INPUT(input_decl, rev):
    cache_key = HASH(input_decl.url + rev)
    cache_path = "~/.cache/syslua/inputs/{cache_key}"

    IF cache_path EXISTS:
        RETURN cache_path

    SWITCH input_decl.type:
        CASE "github", "gitlab":
            tarball_url = CONSTRUCT_ARCHIVE_URL(input_decl, rev)
            DOWNLOAD(tarball_url, cache_path, auth=input_decl.auth)
            EXTRACT(cache_path)

        CASE "git":
            GIT.clone(input_decl.url, cache_path, rev=rev, auth=input_decl.auth)
            REMOVE(cache_path + "/.git")  // Strip git metadata

        CASE "path":
            SYMLINK(input_decl.path, cache_path)

    RETURN cache_path
```

### Lock File Validation Rules

| Scenario                        | Behavior                                 |
| ------------------------------- | ---------------------------------------- |
| Lock exists, input unchanged    | Use locked `rev`                         |
| Lock exists, input URL changed  | Error (must run `sys update`)            |
| Lock exists, input type changed | Error (must run `sys update`)            |
| Lock missing for input          | Resolve latest, add to lock              |
| Lock file missing entirely      | Resolve all inputs, create lock          |
| `sys update` command            | Re-resolve specified inputs, update lock |

## Custom Package Definitions

Users can define custom derivations directly in their `sys.lua`:

```lua
-- Custom derivation from GitHub release (prebuilt binaries)
local hashes = {
    ["x86_64-linux"] = "abc123...",
    ["aarch64-darwin"] = "def456...",
}

local internal_tool_drv = derive {
    name = "my-internal-tool",
    version = "2.1.0",

    opts = function(sys)
        return {
            url = "https://github.com/mycompany/internal-tool/releases/download/v2.1.0/internal-tool-2.1.0-" .. sys.platform .. ".tar.gz",
            sha256 = hashes[sys.platform],
        }
    end,

    config = function(opts, ctx)
        local archive = ctx.fetch_url(opts.url, opts.sha256)
        ctx.unpack(archive, ctx.out)
    end,
}

-- Install it with activation
activate {
    opts = { drv = internal_tool_drv },
    config = function(opts, ctx)
        ctx.add_to_path(opts.drv.out .. "/bin")
    end,
}
```

## See Also

- [Derivations](./01-derivations.md) - How derivations work
- [Activations](./02-activations.md) - How activations work
- [Modules](./07-modules.md) - Module system
