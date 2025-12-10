# Platform Specifics

> Part of the [sys.lua Architecture](./00-overview.md) documentation.

This document covers platform-specific details including services, environment scripts, file management, and paths.

## Platform Abstraction (sys-platform)

The `sys-platform` crate provides OS abstraction for:

- Store/config/cache paths per OS
- Platform identifier (e.g., `aarch64-darwin`)
- Immutability flags (`chflags`, `chattr`, ACLs)
- Environment variable management

## Store Locations

### System Store (managed by admin/root)

| Platform | System Store Path |
| -------- | ----------------- |
| Linux    | `/syslua/store`   |
| macOS    | `/syslua/store`   |
| Windows  | `C:\syslua\store` |

### User Store (managed by each user, no sudo required)

| Platform | User Store Path                              |
| -------- | -------------------------------------------- |
| Linux    | `~/.local/share/syslua/store`                |
| macOS    | `~/Library/Application Support/syslua/store` |
| Windows  | `%LOCALAPPDATA%\syslua\store`                |

## Environment Scripts

### Session Variables

Session variables are written to shell-specific scripts:

| Platform    | Script Location                  | Shell Integration                    |
| ----------- | -------------------------------- | ------------------------------------ |
| Linux/macOS | `~/.local/share/syslua/env.sh`   | Sourced in `.bashrc`/`.zshrc`        |
| Linux/macOS | `~/.local/share/syslua/env.fish` | Sourced in `config.fish`             |
| Windows     | `~/.local/share/syslua/env.ps1`  | Sourced in PowerShell `$PROFILE`     |
| Windows     | `~/.local/share/syslua/env.cmd`  | Via `AutoRun` registry key (cmd.exe) |

```bash
# Unix: env.sh (sourced by user's shell)
export PATH="/path/to/store/pkg/ripgrep/15.1.0/aarch64-darwin:$PATH"
export EDITOR="nvim"
```

```powershell
# Windows: env.ps1 (sourced by PowerShell profile)
$env:PATH = "C:\syslua\store\pkg\ripgrep\15.1.0\x86_64-windows;$env:PATH"
$env:EDITOR = "nvim"
```

**Shell integration (user adds to their config):**

```bash
# Unix: ~/.bashrc or ~/.zshrc
[ -f ~/.local/share/syslua/env.sh ] && source ~/.local/share/syslua/env.sh
```

```powershell
# Windows: $PROFILE
if (Test-Path "$env:USERPROFILE\.local\share\sys\env.ps1") {
    . "$env:USERPROFILE\.local\share\sys\env.ps1"
}
```

### Per-User Profiles

sys.lua generates **separate environment scripts for each user** defined in the configuration:

```
~/.local/share/syslua/
├── env.sh              # System-level env (all users)
├── env.fish
├── env.ps1
└── users/
    ├── ian/
    │   ├── env.sh      # ian's packages + env vars
    │   ├── env.fish
    │   └── env.ps1
    └── admin/
        ├── env.sh      # admin's packages + env vars
        ├── env.fish
        └── env.ps1
```

**How it works:**

1. System-level `setup()` and `env{}` go into the root env scripts
2. User-scoped declarations (inside `user { config = ... }`) go into per-user scripts
3. Users source both: system env first, then their user env
4. User env can override/extend system env

### Persistent Variables

Persistent variables are written directly to OS-level configuration, available to all processes (including GUI apps and services):

| Platform | System Location                   | User Location                             |
| -------- | --------------------------------- | ----------------------------------------- |
| Linux    | `/etc/environment`                | `~/.pam_environment`                      |
| macOS    | `/etc/launchd.conf` + `launchctl` | `~/Library/LaunchAgents/syslua.env.plist` |
| Windows  | Registry `HKLM\...\Environment`   | Registry `HKCU\Environment`               |

**Why Registry for Windows persistent vars (not PowerShell profile):**

- Registry is the canonical location for Windows environment variables
- Available to all processes: GUI apps, services, scheduled tasks, all shells
- PowerShell profiles only affect PowerShell sessions
- `env.persistent {}` semantics require system-wide visibility

**Rollback behavior:** Persistent variables are tracked in snapshots and restored during rollback.

## File Management

sys.lua provides declarative file management through the unified derivation model.

**Important: Files are fully managed by sys.lua.** When you declare a file:

- The file's entire content is replaced with what you specify
- Existing content is NOT preserved or merged
- Removing a file declaration removes the file from disk
- Changes made outside sys.lua will be overwritten on next `sys apply`

### File Modes

| Mode                   | Description                                  | Store Behavior         | Use Case                         |
| ---------------------- | -------------------------------------------- | ---------------------- | -------------------------------- |
| Store-backed (default) | Content copied to store, symlinked to target | Immutable in store     | Config files, dotfiles           |
| Mutable                | Direct symlink to source                     | Metadata only in store | Files that need in-place editing |

### Store-Backed vs Mutable

**Store-backed (default):**

```
~/.gitconfig → ~/.local/share/syslua/store/obj/file-gitconfig-abc123/content
```

- Content is immutable in the store
- Editing `~/.gitconfig` directly will fail (read-only)
- Changes require updating config and running `sys apply`
- Automatic rollback via generations

**Mutable:**

```
~/.gitconfig → /absolute/path/to/dotfiles/gitconfig
```

- Direct symlink, content lives outside the store
- File can be edited in place
- Still tracked by sys.lua (derivation records the link metadata)
- No content-based rollback (rollback restores the symlink, not content)

## Service Management

sys.lua provides cross-platform declarative service management using native init systems.

### Platform Backends

| Platform | Init System                       | Service Location                                     |
| -------- | --------------------------------- | ---------------------------------------------------- |
| Linux    | systemd                           | `/etc/systemd/system/`                               |
| macOS    | launchd                           | `/Library/LaunchDaemons/`, `~/Library/LaunchAgents/` |
| Windows  | Windows Services + Task Scheduler | Registry / Task Scheduler                            |

### Declaring Services

Services are modules, not a special global:

```lua
-- Simple service
require("modules.services.nginx").setup()

-- With options
require("modules.services.nginx").setup({
    port = 8080,
    workers = 4,
})

-- Custom service module example
-- modules/services/myapp/init.lua
local M = {}

M.options = {
    port = 3000,
}

function M.setup(opts)
    opts = opts or {}
    for k, v in pairs(M.options) do
        if opts[k] == nil then opts[k] = v end
    end
    
    -- Derive the service unit file
    local service_drv = derive {
        name = "myapp-service",
        opts = function(sys) return { sys = sys, port = opts.port } end,
        config = function(o, ctx)
            if o.sys.os == "linux" then
                ctx.write(ctx.out .. "/myapp.service", [[
[Unit]
Description=My Application
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/myapp --port=]] .. o.port .. [[

Restart=always
User=myapp

[Install]
WantedBy=multi-user.target
]])
            elseif o.sys.os == "macos" then
                ctx.write(ctx.out .. "/myapp.plist", generate_launchd_plist(o))
            end
        end,
    }
    
    -- Activate: install and enable
    activate {
        opts = function(sys) return { drv = service_drv, sys = sys } end,
        config = function(o, ctx)
            if o.sys.os == "linux" then
                ctx.symlink(o.drv.out .. "/myapp.service", "/etc/systemd/system/myapp.service")
                ctx.enable_service("myapp")
            elseif o.sys.os == "macos" then
                ctx.symlink(o.drv.out .. "/myapp.plist", "~/Library/LaunchAgents/myapp.plist")
                ctx.enable_service("myapp")
            end
        end,
    }
    
    return M
end

return M
```

### User Services

Services can be scoped to users:

```lua
user {
    name = "ian",
    config = function()
        require("modules.services.syncthing").setup()
        -- Service runs as ian, not root
    end,
}
```

## Immutability

Objects in `obj/<hash>/` are made immutable after extraction:

**System store objects:**

- **Linux:** `chattr +i` (requires root to modify)
- **macOS:** `chflags uchg` (requires root to modify)
- **Windows:** ACL restrictions (requires admin to modify)
- **World-readable:** `chmod 755` (directories), `chmod 644` (files)

**User store objects:**

- **Same immutability flags** (user owns them, but makes immutable)
- **Purpose:** Prevent accidental modification
- **Removal:** User can run `sys gc` to remove (clears immutability first)

## Build System

While sys.lua prefers prebuilt binaries for speed, it supports building from source when necessary.

### Prebuilt vs Source

```lua
-- Prebuilt binary (preferred, fast)
derive {
    name = "ripgrep",
    version = "15.1.0",
    config = function(opts, ctx)
        local archive = ctx.fetch_url(opts.url, opts.sha256)
        ctx.unpack(archive, ctx.out)
    end,
}

-- Build from source (when no prebuilt available)
derive {
    name = "custom-tool",
    version = "1.0.0",
    opts = function(sys)
        return {
            rust = rust_drv,  -- Build dependency
        }
    end,
    config = function(opts, ctx)
        local src = ctx.fetch_git(opts.git_url, opts.rev, opts.sha256)
        ctx.env.PATH = opts.rust.out .. "/bin:" .. ctx.env.PATH
        ctx.run("cargo build --release", { cwd = src })
        ctx.mkdir(ctx.out .. "/bin")
        ctx.copy(src .. "/target/release/custom-tool", ctx.out .. "/bin/custom-tool")
    end,
}
```

### Cross-Compilation (Future)

Cross-compilation is **not supported in the initial release**. Current recommendation:

1. Use prebuilt binaries from releases (preferred)
2. Build natively on each target platform (CI/CD)
3. Use Docker/VMs for foreign platform builds

## See Also

- [Store](./03-store.md) - Store layout and deduplication
- [Derivations](./01-derivations.md) - Build system details
- [Snapshots](./05-snapshots.md) - Rollback for services and files
