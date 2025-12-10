# Apply Flow

> Part of the [sys.lua Architecture](./00-overview.md) documentation.

This document covers the apply command flow, DAG construction, parallel execution, and atomicity.

## Overview

The apply command is fully declarative - it makes the current state match the config exactly by both installing new packages and removing packages not in the config.

**Key Design Principle:** Lua configuration is evaluated into a manifest first, conflicts are resolved using priorities, then a DAG-based system applies changes. This ensures:

- Order of declarations in Lua does not affect the final result
- Conflicts are detected and resolved deterministically
- The system determines optimal execution order, not the user
- Dependencies are resolved before dependents
- Parallel execution where possible

## Apply Flow Diagram

```
sys apply sys.lua
    │
    ├─► PHASE 1: EVALUATION
    │   ├─► Parse sys.lua with Lua runtime
    │   ├─► Execute all require().setup(), file{}, env{}, user{} declarations
    │   ├─► Collect all declarations with their priorities
    │   └─► Resolve fetch helpers (fetchUrl, fetchGit, etc.)
    │
    ├─► PHASE 2: MERGE & CONFLICT RESOLUTION
    │   ├─► Group declarations by key (package name, file path, env var)
    │   ├─► For each group:
    │   │   ├─► Singular values: lowest priority wins
    │   │   ├─► Mergeable values: combine and sort by priority
    │   │   └─► Same priority + different values: ERROR
    │   └─► Produce resolved Manifest
    │
    ├─► PHASE 3: PLANNING
    │   ├─► Load registry from effective path
    │   ├─► Get current installed state from store
    │   ├─► Compute diff: desired (manifest) vs current
    │   │   ├─► to_install = desired - current
    │   │   └─► to_remove = current - desired
    │   ├─► Build execution DAG from manifest
    │   │   ├─► Nodes: packages, files, env vars
    │   │   └─► Edges: depends_on relationships
    │   └─► Topologically sort DAG for execution order
    │
    ├─► PHASE 4: EXECUTION
    │   ├─► Display plan (always shown)
    │   ├─► If no changes: exit early
    │   ├─► Create pre-apply snapshot (with config content)
    │   ├─► Execute DAG in topological order:
    │   │   ├─► Parallel execution for independent nodes
    │   │   ├─► Download/verify/extract packages
    │   │   ├─► Create/update files
    │   │   └─► Update environment
    │   ├─► On failure: rollback completed nodes, abort
    │   ├─► Create post-apply snapshot (with config content)
    │   └─► Generate env scripts (env.sh, env.fish)
    │
    └─► Print summary and shell setup instructions
```

## Manifest Structure

The manifest is the intermediate representation between Lua config and system state:

```rust
pub struct Manifest {
    pub packages: Vec<PackageSpec>,
    pub files: Vec<FileSpec>,
    pub env: EnvConfig,
    pub users: Vec<UserConfig>,
}

pub struct PackageSpec {
    pub name: String,
    pub version: String,
    pub source: Source,           // Resolved from fetch helpers
    pub bin: Vec<String>,
    pub depends_on: Vec<String>,  // Package dependencies
}

pub enum Source {
    Url { url: String, sha256: String },
    Git { url: String, rev: String, sha256: String },
    GitHub { owner: String, repo: String, tag: String, asset: String, sha256: String },
}
```

## Execution DAG

The DAG ensures correct ordering regardless of config declaration order:

```
Example: User declares in any order:
  require("pkgs.cli.neovim").setup()
  require("pkgs.cli.ripgrep").setup()
  file { path = "~/.config/nvim/init.lua", ... }  -- depends on neovim

DAG constructed:
  ┌──────────┐     ┌──────────┐
  │ ripgrep  │     │  neovim  │
  └──────────┘     └────┬─────┘
                        │ depends_on
                        ▼
                  ┌───────────────┐
                  │ nvim/init.lua │
                  └───────────────┘

Execution order (determined by system, not user):
  1. ripgrep, neovim (parallel - no dependencies between them)
  2. nvim/init.lua (after neovim completes)
```

### DAG Execution Example

```
$ sys plan init.lua

Changes:
  + ripgrep@15.1.0
  + neovim@0.10.0
  + postgresql@16.1.0 (package)
  + postgresql@16.1.0 (service)
  + ~/.config/nvim/init.lua

Execution order:
  [Wave 1] ripgrep, neovim, postgresql (package) - parallel
  [Wave 2] nvim/init.lua, postgresql (service) - parallel (after wave 1)
```

## Atomic Apply (All-or-Nothing)

**sys.lua uses atomic semantics for the apply operation.** Either all changes succeed or the system remains in its previous state - there is no partial application.

### Why Atomic?

Partial application creates broken states that are difficult to debug and recover from:

- A file might reference a package that failed to install
- Environment variables might point to missing paths
- Services might fail because their dependencies aren't available
- Users would need to manually figure out what succeeded vs failed

### How It Works

```
Apply begins
    │
    ├─► Create pre-apply snapshot
    │
    ├─► Execute DAG nodes...
    │       │
    │       ├─► Node 1: Success ✓ (tracked)
    │       ├─► Node 2: Success ✓ (tracked)
    │       ├─► Node 3: FAILURE ✗
    │       │
    │       └─► Rollback triggered
    │               │
    │               ├─► Undo Node 2
    │               ├─► Undo Node 1
    │               └─► Restore pre-apply snapshot
    │
    └─► Exit with error (system unchanged)
```

### Rollback Behavior

When any node in the DAG fails:

1. **Stop execution** - No further nodes are attempted
2. **Undo completed nodes** - In reverse order of completion
3. **Restore snapshot** - Revert to the pre-apply snapshot
4. **Report failure** - Show which node failed and why

```bash
$ sudo sys apply sys.lua
Evaluating sys.lua...
Building execution plan...

Executing:
  [1/4] ✓ ripgrep@15.1.0
  [2/4] ✓ fd@9.0.0
  [3/4] ✗ custom-tool@1.0.0
        Error: Build failed: missing dependency 'libfoo'

Rolling back...
  - Removing fd@9.0.0 from profile
  - Removing ripgrep@15.1.0 from profile
  - Restoring pre-apply state

Apply failed. System unchanged.
Run 'sys plan' to review the execution plan.
```

### What Gets Rolled Back

| Component       | Rollback Action                                        |
| --------------- | ------------------------------------------------------ |
| **Packages**    | Remove from `pkg/` symlinks (objects remain in `obj/`) |
| **Files**       | Restore from pre-apply snapshot backup                 |
| **Symlinks**    | Restore original target or remove                      |
| **Environment** | Regenerate env scripts from previous state             |
| **Services**    | Stop newly started services, restart stopped services  |

### Edge Cases

**Already-installed packages**: If a package already exists in the store from a previous apply, it's not re-downloaded. Rollback simply removes the symlink - the cached object remains for future use.

**External changes during apply**: If the system is modified externally during apply (rare), rollback restores to the snapshot which reflects state at apply-start, not the external changes.

**Idempotent re-apply**: After a failed apply and rollback, running `sys apply` again will attempt the same changes. Fix the underlying issue (e.g., the missing `libfoo` dependency) before re-running.

## Plan Command

Preview changes without applying (evaluates config to manifest, builds DAG, but doesn't execute):

```bash
$ sys plan sys.lua
Evaluating sys.lua...
Building execution plan...

Install:
  + fd@9.0.0
  + bat@0.24.0
Remove:
  - ripgrep@14.1.1
Unchanged:
  = jq@1.7.1

Execution order:
  1. [parallel] fd@9.0.0, bat@0.24.0
  2. [remove] ripgrep@14.1.1
```

## Priority-Based Conflict Resolution

When multiple declarations affect the same key, priorities determine the outcome:

### Priority Values

| Function        | Priority | Use Case                    |
| --------------- | -------- | --------------------------- |
| `lib.mkForce`   | 50       | Force a value (highest)     |
| `lib.mkBefore`  | 500      | Prepend to mergeable values |
| (default)       | 1000     | Normal declarations         |
| `lib.mkDefault` | 1000     | Provide a default           |
| `lib.mkAfter`   | 1500     | Append to mergeable values  |

### Singular Values

For values that can only have one result (e.g., `EDITOR`), lowest priority wins:

```lua
env { EDITOR = "vim" }                    -- priority 1000
env { EDITOR = lib.mkDefault("nano") }    -- priority 1000 (same)
env { EDITOR = lib.mkForce("nvim") }      -- priority 50 (wins)
```

### Mergeable Values

For values that combine (e.g., `PATH`), all declarations are merged and sorted by priority:

```lua
env { PATH = lib.mkBefore("/custom/bin") }  -- priority 500 (first)
env { PATH = "/home/user/bin" }              -- priority 1000 (middle)
env { PATH = lib.mkAfter("/opt/bin") }       -- priority 1500 (last)
-- Result: PATH="/custom/bin:/home/user/bin:/opt/bin:$PATH"
```

### Conflict Errors

Same priority + different values = error:

```lua
env { EDITOR = "vim" }   -- priority 1000
env { EDITOR = "emacs" } -- priority 1000 (ERROR!)
```

```
Error: Conflicting values for env.EDITOR at priority 1000:
  - "vim" (declared at sys.lua:10)
  - "emacs" (declared at sys.lua:15)

Use lib.mkForce() to override, or lib.mkDefault() to provide a fallback.
```

## See Also

- [Derivations](./01-derivations.md) - Build recipes
- [Activations](./02-activations.md) - Making derivations visible
- [Snapshots](./05-snapshots.md) - State capture and rollback
- [Store](./03-store.md) - Where objects live
