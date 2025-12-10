# Activations

> **Core Principle:** Activations describe what to do with derivation outputs.

While derivations are pure build artifacts (content in the store), activations specify how those outputs should be made visible to the user and system.

This separation follows Nix's model and provides:

- **Better caching**: Same content with different targets = one derivation, multiple activations
- **Composability**: Future features (services, programs, bootloader configs) use the same pattern
- **Cleaner rollback**: "Same derivations, different activations" is a clear, understandable diff
- **Separation of concerns**: Build logic stays in derivations; deployment logic in activations

## The Two Building Blocks

```
┌─────────────────────────────────────────────────────────────────┐
│  Derivation                                                     │
│  ═══════════                                                    │
│  Pure build artifact. Describes HOW to produce content.         │
│  Cached by hash in store/obj/<name>-<hash>/                     │
│  Immutable once built. Same inputs → same output.               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ produces
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Activation                                                     │
│  ══════════                                                     │
│  Describes WHAT TO DO with derivation output.                   │
│  Where to symlink, which services to enable, PATH additions.    │
│  Multiple activations can reference the same derivation.        │
└─────────────────────────────────────────────────────────────────┘
```

## Activation Types

An activation describes an action to take with a derivation's output:

```rust
/// An activation describes what to do with a derivation output.
pub struct Activation {
    /// The derivation hash this activation references
    pub derivation_hash: String,
    /// Which output to use (usually "out")
    pub output: String,
    /// The action to perform
    pub action: ActivationAction,
}

/// Actions that can be performed with derivation outputs.
pub enum ActivationAction {
    /// Create a symlink from target to the derivation output (or subpath within it)
    Symlink {
        target: PathBuf,
        subpath: Option<String>,  // e.g., "/content" for file derivations
        mutable: bool,            // Direct symlink vs store-backed
    },

    /// Add the derivation's bin directory to PATH
    AddToPath {
        bin_subdir: Option<String>,  // Defaults to "bin"
    },

    /// Source a script from the derivation in shell init
    SourceInShell {
        shells: Vec<Shell>,          // bash, zsh, fish, powershell
        script_subpath: String,      // e.g., "env.sh"
    },

    /// Enable/manage a system service (future)
    Service {
        service_type: ServiceType,   // systemd, launchd, windows
        enable: bool,
    },

    /// Write to bootloader configuration (future)
    BootloaderEntry {
        entry_type: String,
    },
}

pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}
```

## The `activate {}` Function

Activations follow the same `opts`/`config` pattern as derivations:

```lua
activate {
    opts = function(sys)
        return { ... }  -- Any data needed by config function
    end,
    config = function(opts, ctx)
        -- ctx.sys: System table
        -- ctx.add_to_path(path)
        -- ctx.symlink(source, target, opts?)
        -- ctx.source_in_shell(script, opts?)
        -- ctx.run(cmd, opts?)  -- Escape hatch
    end,
}
```

## Activation Context (`ActivationCtx`)

```lua
---@class ActivationCtx
---@field sys System                    -- Platform info (same as DerivationCtx.sys)

-- Activation actions
---@field add_to_path fun(bin_path: string): nil
---@field symlink fun(source: string, target: string, opts?: SymlinkOpts): nil
---@field source_in_shell fun(script: string, opts?: SourceOpts): nil

-- Escape hatch (logs warning during plan phase)
---@field run fun(cmd: string, opts?: RunOpts): nil

---@class SymlinkOpts
---@field mutable? boolean     -- Direct symlink, not store-backed

---@class SourceOpts
---@field shells? string[]     -- Restrict to specific shells: "bash", "zsh", "fish", "powershell"
```

## How User APIs Map to Derivations + Activations

### Package Installation via `setup()`

```lua
require("inputs.pkgs.cli.ripgrep").setup()
```

Creates:

- **Derivation**: Created by `make_derivation()` - the ripgrep build
- **Activation**: Created by `make_activation(drv)` - `AddToPath { bin_subdir: None }` - adds `<drv>/bin` to PATH

### `file {}` - File Management

```lua
file { path = "~/.gitconfig", source = "./dotfiles/gitconfig" }
```

Creates:

- **Derivation**: Copies source content to `<out>/content`
- **Activation**: `Symlink { target: "~/.gitconfig", subpath: Some("/content"), mutable: false }`

### `file {}` with Mutable Mode

```lua
file { path = "~/.gitconfig", source = "./dotfiles/gitconfig", mutable = true }
```

Creates:

- **Derivation**: Stores metadata about the link (for tracking)
- **Activation**: `Symlink { target: "~/.gitconfig", subpath: None, mutable: true }` - direct symlink to source

### `env {}` - Environment Variables

```lua
env { EDITOR = "nvim" }
```

Creates:

- **Derivation**: Generates shell fragments (`env.sh`, `env.fish`, `env.ps1`)
- **Activation**: `SourceInShell { shells: [Bash, Zsh, Fish, PowerShell], script_subpath: "env.sh" }`

## Examples

### Simple Package Activation

```lua
activate {
    opts = function(sys)
        return { drv = inputs.pkgs.ripgrep.derivation }
    end,
    config = function(opts, ctx)
        ctx.add_to_path(opts.drv.out .. "/bin")
    end,
}
```

### Multiple Activations from Same Derivation

```lua
local my_tool = derive { name = "my-tool", ... }

-- Add to PATH
activate {
    opts = function(sys) return { drv = my_tool } end,
    config = function(opts, ctx)
        ctx.add_to_path(opts.drv.out .. "/bin")
    end,
}

-- Also create symlinks for shared resources
activate {
    opts = function(sys) return { drv = my_tool } end,
    config = function(opts, ctx)
        ctx.symlink(opts.drv.out .. "/share/man", "~/.local/share/man/my-tool")
        ctx.symlink(opts.drv.out .. "/completions/zsh", "~/.zsh/completions/_mytool")
    end,
}
```

### Platform-Specific Activation

```lua
activate {
    opts = function(sys)
        return { drv = inputs.pkgs.neovim.derivation }
    end,
    config = function(opts, ctx)
        ctx.add_to_path(opts.drv.out .. "/bin")

        if ctx.sys.is_darwin then
            ctx.symlink(opts.drv.out .. "/Applications/Neovim.app", "~/Applications/Neovim.app")
        end
    end,
}
```

### Pure Activation Scripts (No Derivation)

Like home-manager's activation scripts, you can run arbitrary commands:

```lua
activate {
    config = function(opts, ctx)
        if ctx.sys.is_darwin then
            ctx.run("defaults write com.apple.dock autohide -bool true")
            ctx.run("killall Dock")
        end
    end,
}
```

### Environment Script Sourcing

```lua
activate {
    opts = function(sys) return { drv = my_env_drv } end,
    config = function(opts, ctx)
        ctx.source_in_shell(opts.drv.out .. "/env.sh", { shells = {"bash", "zsh"} })
        ctx.source_in_shell(opts.drv.out .. "/env.fish", { shells = {"fish"} })
    end,
}
```

## Package Module Pattern

Packages use a **factory function pattern** that enables explicit control over derivation and activation creation:

### Versioned Package File (`pkgs/cli/ripgrep/15_1_0.lua`)

```lua
---@class pkgs.cli.ripgrep.15_1_0
local M = {}

local hashes = {
    ["aarch64-darwin"] = "abc123...",
    ["x86_64-linux"] = "def456...",
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

### User Customization

```lua
-- Override factory functions before calling setup
local rg = require("inputs.pkgs.cli.ripgrep")["15_1_0"]

rg.make_activation = function(drv)
    return activate({
        opts = { drv = drv },
        config = function(opts, ctx)
            ctx.add_to_path(opts.drv.out .. "/bin")
            ctx.symlink(opts.drv.out .. "/doc", "~/.local/share/doc/ripgrep")
        end,
    })
end

rg.setup()

-- Or skip activation entirely
local openssl = require("inputs.pkgs.cli.openssl")["3_0_0"]
openssl.make_activation = function() end  -- no-op
openssl.setup()
```

**Why factory functions:**

1. **No magic** - `drv` is passed explicitly to `make_activation`
2. **Overridable** - Users can replace factory functions before calling `setup()`
3. **Inspectable** - Users can call `make_derivation()` to see what it returns
4. **Clear data flow** - `derive()` returns something, that something is passed to `activate()`

## When to Use Explicit Activations

Use the `activate {}` function when:

1. **Custom derivations**: Derivations not from standard packages
2. **Multiple activations**: Same derivation needs multiple symlinks
3. **Platform-specific logic**: Different behavior per OS
4. **Library packages**: Install derivation without PATH activation
5. **Activation scripts**: System configuration commands (like home-manager)
6. **Service management**: Platform-specific service setup

## Rust Types for Activations

```rust
/// Options specification for activations
pub enum ActivateOptsSpec {
    Static(HashMap<String, OptsValue>),
    Dynamic(LuaFunction),  // fn(sys) -> HashMap<String, OptsValue>
}

/// The activation specification
pub struct Activation {
    pub opts: Option<ActivateOptsSpec>,
    pub config: LuaFunction,  // fn(opts, ctx)
    pub hash: ActivationHash,
}

/// Activation context provided to config function
pub struct ActivationCtx {
    pub sys: System,
    // Methods implemented via mlua UserData
}

/// Collected activation actions (from running config function)
pub enum ActivationAction {
    AddToPath { bin_path: PathBuf },
    Symlink { source: PathBuf, target: PathBuf, mutable: bool },
    SourceInShell { script: PathBuf, shells: Vec<Shell> },
    Run { cmd: String, opts: Option<RunOpts> },
}
```

## Why This Matters for Snapshots

With derivations and activations as separate concepts, snapshots become much simpler:

```rust
/// A snapshot captures system state as derivation hashes + activations.
pub struct Snapshot {
    pub id: String,
    pub created_at: u64,
    pub description: String,

    /// Just the hashes of derivations in this snapshot
    pub derivations: Vec<String>,

    /// The activations that make these derivations visible
    pub activations: Vec<Activation>,

    /// Configuration that produced this state (for reproducibility)
    pub config_path: Option<PathBuf>,
}
```

**Benefits:**

1. **No separate types**: No need for `SnapshotFile`, `SnapshotEnv`, `SnapshotDerivation` - just derivation hashes and activations
2. **Clear diffs**: Comparing snapshots shows exactly what changed:
   - Same derivations, different activations = only deployment changed
   - Different derivations, same activations = content changed
3. **GC-safe**: Derivations referenced by any snapshot are protected from garbage collection
4. **Future-proof**: New activation types (services, bootloader) slot in naturally

## Related Documentation

- [01-derivations.md](./01-derivations.md) - How derivations produce content
- [03-store.md](./03-store.md) - Where derivation outputs live
- [05-snapshots.md](./05-snapshots.md) - How activations enable rollback
