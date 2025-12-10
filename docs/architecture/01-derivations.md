# Derivations

> **Core Principle:** Derivations are the sole primitive for producing store content in sys.lua.

A derivation is an immutable description of:
- What inputs are needed (arbitrary data)
- How to transform those inputs into outputs (build function)
- What outputs are produced

All managed state in sys.lua builds on derivations - not just packages, but also files and environment variables.

## The `derive {}` Function

```lua
local my_drv = derive {
  name = "ripgrep",           -- Required: identifier for debugging/logging
  version = "15.1.0",         -- Optional: human-readable version
  outputs = {"out"},          -- Optional: defaults to {"out"}

  opts = <table | function(sys)>,       -- Optional: input specification
  config = function(opts, ctx),         -- Required: build logic
}
```

## Options (`opts`)

Options can be a static table or a function for platform-specific resolution. **Options are arbitrary data** - there is no magic interpretation. The `config` function consumes this data and uses `ctx` helpers as needed.

```lua
-- Static table (simple case)
opts = {
  src_url = "https://example.com/tool.tar.gz",
  src_sha256 = "abc123...",
  settings = { feature = true },
}

-- Function for cross-platform (receives sys table)
opts = function(sys)
  return {
    src_url = "https://example.com/tool-" .. sys.platform .. ".tar.gz",
    src_sha256 = hashes[sys.platform],
  }
end
```

### Derivation References

Options can include other derivations for build dependencies:

```lua
local rust = derive { name = "rust", ... }

derive {
  name = "ripgrep",
  opts = function(sys)
    return {
      src_url = "...",
      rust = rust,  -- Derivation reference
    }
  end,
  config = function(opts, ctx)
    -- opts.rust.out is the realized path of the rust derivation
    ctx.env.PATH = opts.rust.out .. "/bin:" .. ctx.env.PATH
    ...
  end,
}
```

## Config Function

The config function transforms options into outputs:

```lua
config = function(opts, ctx)
  -- opts: the table returned by opts function (derivation refs have .out paths)
  -- ctx: build context with helpers (includes ctx.sys for platform info)
end
```

## Derivation Context (`DerivationCtx`)

The derivation context provides system information, output paths, and helpers for fetching, filesystem operations, and shell execution:

```lua
-- System information
ctx.sys.platform   -- "aarch64-darwin", "x86_64-linux", "x86_64-windows"
ctx.sys.os         -- "darwin" | "linux" | "windows"
ctx.sys.arch       -- "aarch64" | "x86_64" | "arm"
ctx.sys.hostname   -- Machine hostname
ctx.sys.username   -- Current user
ctx.sys.is_darwin  -- Convenience boolean
ctx.sys.is_linux   -- Convenience boolean
ctx.sys.is_windows -- Convenience boolean

-- Output paths
ctx.out                -- Primary output directory
ctx.outputs.out        -- Same as ctx.out
ctx.outputs.<name>     -- For multi-output (future)

-- Fetch operations (immediate, not derivations)
ctx.fetch_url(url, sha256, opts?)      -- Download file, verify hash, return path
ctx.fetch_git(url, rev, sha256, opts?) -- Clone repo, checkout rev, return path

-- Filesystem operations
ctx.unpack(archive, dest?)    -- Extract tar.gz/zip/etc to dest (default: ctx.out)
ctx.copy(src, dst)            -- Copy file or directory
ctx.move(src, dst)            -- Move file or directory
ctx.mkdir(path)               -- Create directory (recursive)
ctx.write(path, content)      -- Write string to file
ctx.chmod(path, mode)         -- Set permissions (unix)
ctx.symlink(target, link)     -- Create symbolic link

-- Shell execution (use sparingly)
ctx.run(cmd, opts?)           -- Run shell command
                              -- opts: { cwd, env, shell }
                              -- Default shell: sh (unix), powershell (windows)

-- Environment for ctx.run
ctx.env                       -- Mutable table, seeded with basic PATH
```

**Error handling:** All `ctx` operations throw on failure (Lua `error()`). A failed build leaves the user-facing system unchanged - atomic apply semantics ensure the pre-apply state is restored.

## Derivation Return Value

`derive {}` returns a table representing the derivation AND registers it globally. The registration happens on require - users can conditionally require modules for platform-specific packages.

```lua
local rg = derive { name = "ripgrep", outputs = {"out"}, ... }

rg.name           -- "ripgrep"
rg.version        -- "15.1.0" or nil
rg.hash           -- Derivation hash (computed at evaluation time)
rg.outputs        -- {"out"}

-- Output paths (available after realization)
rg.out            -- "/syslua/store/obj/ripgrep-15.1.0-<hash>"
rg.outputs.out    -- Same as rg.out
```

## Derivation Hashing

The derivation hash is computed from:

- `name`
- `version` (if present)
- `opts` (evaluated result, including sys)
- `config` function source code hash
- `outputs` list
- `sys` (platform, os, arch)

This means:

- Same opts + different config function = different derivation
- Same derivation on different platforms = different hash
- Derivation dependencies are included via their hash in opts

## Rust Types

```rust
/// Options specification - either static or requires sys for resolution
pub enum OptsSpec {
    Static(HashMap<String, OptsValue>),
    Dynamic(LuaFunction),  // fn(sys) -> HashMap<String, OptsValue>
}

/// Values that can appear in opts
pub enum OptsValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Table(HashMap<String, OptsValue>),
    Array(Vec<OptsValue>),
    Derivation(DerivationRef),
}

/// Reference to another derivation
pub struct DerivationRef {
    pub hash: DerivationHash,
    pub outputs: HashMap<String, PathBuf>,
}

/// The derivation specification
pub struct Derivation {
    pub name: String,
    pub version: Option<String>,
    pub opts: Option<OptsSpec>,
    pub config: LuaFunction,  // fn(opts, ctx)
    pub outputs: Vec<String>,
    pub hash: DerivationHash,
}

/// Platform information available via ctx.sys
pub struct System {
    pub platform: String,  // "aarch64-darwin"
    pub os: Os,            // darwin | linux | windows
    pub arch: Arch,        // aarch64 | x86_64 | arm
    pub hostname: String,
    pub username: String,
    pub is_darwin: bool,
    pub is_linux: bool,
    pub is_windows: bool,
}

/// Derivation context provided to config function
pub struct DerivationCtx {
    pub sys: System,
    pub out: PathBuf,
    pub outputs: HashMap<String, PathBuf>,
    pub env: HashMap<String, String>,
    // Methods implemented via mlua UserData
}
```

## Examples

### Prebuilt Binary

```lua
local hashes = {
  ["aarch64-darwin"] = "abc...",
  ["x86_64-linux"] = "def...",
}

local ripgrep = derive {
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
}
```

### Build from Source

```lua
local rust = derive { name = "rust", ... }

local ripgrep = derive {
  name = "ripgrep",
  version = "15.1.0",

  opts = function(sys)
    return {
      git_url = "https://github.com/BurntSushi/ripgrep",
      rev = "15.1.0",
      sha256 = "source-hash...",
      rust = rust,
    }
  end,

  config = function(opts, ctx)
    local src = ctx.fetch_git(opts.git_url, opts.rev, opts.sha256)

    ctx.env.PATH = opts.rust.out .. "/bin:" .. ctx.env.PATH
    ctx.run("cargo build --release", { cwd = src })

    ctx.mkdir(ctx.out .. "/bin")
    ctx.copy(src .. "/target/release/rg", ctx.out .. "/bin/rg")
  end,
}
```

### Platform-Specific Build Logic

```lua
derive {
  name = "my-tool",

  opts = function(sys)
    return {
      url = "https://example.com/my-tool-" .. sys.platform .. ".tar.gz",
      sha256 = hashes[sys.platform],
    }
  end,

  config = function(opts, ctx)
    local archive = ctx.fetch_url(opts.url, opts.sha256)
    ctx.unpack(archive, ctx.out)

    if ctx.sys.is_darwin then
      -- macOS-specific post-processing
      ctx.run("install_name_tool -id @rpath/libfoo.dylib " .. ctx.out .. "/lib/libfoo.dylib")
    elseif ctx.sys.is_linux then
      -- Linux-specific
      ctx.run("patchelf --set-rpath '$ORIGIN' " .. ctx.out .. "/lib/libfoo.so")
    end
  end,
}
```

## File and Env Derivations

Every `file {}` and `env {}` declaration internally creates a derivation:

### File Derivations

```lua
-- User writes:
file { path = "~/.gitconfig", source = "./dotfiles/gitconfig" }

-- Internally becomes:
local file_drv = derive {
    name = "file-gitconfig",
    opts = { source = "./dotfiles/gitconfig" },
    config = function(opts, ctx)
        ctx.copy(opts.source, ctx.out .. "/content")
    end,
}

activate {
    opts = { drv = file_drv, target = "~/.gitconfig" },
    config = function(opts, ctx)
        ctx.symlink(opts.drv.out .. "/content", opts.target)
    end,
}
```

### Env Derivations

```lua
-- User writes:
env { EDITOR = "nvim", PAGER = "less" }

-- Internally becomes:
local env_drv = derive {
    name = "env-editor-pager",
    opts = { vars = { EDITOR = "nvim", PAGER = "less" } },
    config = function(opts, ctx)
        -- Generate shell-specific fragments
        ctx.write(ctx.out .. "/env.sh", [[export EDITOR="nvim"
export PAGER="less"]])
        ctx.write(ctx.out .. "/env.fish", [[set -gx EDITOR "nvim"
set -gx PAGER "less"]])
        ctx.write(ctx.out .. "/env.ps1", [[$env:EDITOR = "nvim"
$env:PAGER = "less"]])
    end,
}

activate {
    opts = { drv = env_drv },
    config = function(opts, ctx)
        ctx.source_in_shell(opts.drv.out .. "/env.sh", { shells = {"bash", "zsh"} })
        ctx.source_in_shell(opts.drv.out .. "/env.fish", { shells = {"fish"} })
        ctx.source_in_shell(opts.drv.out .. "/env.ps1", { shells = {"powershell"} })
    end,
}
```

## Benefits of Unified Derivation Model

| Aspect                 | Direct Management | Derivation-Based          |
| ---------------------- | ----------------- | ------------------------- |
| Content deduplication  | None              | Automatic                 |
| Rollback               | Manual tracking   | Free via generations      |
| Reproducibility        | Best-effort       | Guaranteed                |
| Atomic apply           | Complex           | Natural                   |
| Cross-file consistency | Must coordinate   | Store ensures consistency |

## Related Documentation

- [02-activations.md](./02-activations.md) - What to do with derivation outputs
- [03-store.md](./03-store.md) - Where derivations are realized
- [08-apply-flow.md](./08-apply-flow.md) - How derivations are built during apply
