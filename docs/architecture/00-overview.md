# sys.lua Architecture Overview

> **Note:** This is a design document describing the target architecture for sys.lua.

sys.lua is a cross-platform declarative system/environment manager inspired by Nix.

## Core Values

1. **Standard Lua Idioms**: Plain tables, functions, `require()`. No magic, no DSL, no hidden behavior.
2. **Reproducibility**: Same config + same inputs = same environment, regardless of platform
3. **Derivations & Activations**: The two atomic building blocks upon which all user-facing APIs are built
4. **Immutability**: Store objects are immutable and content-addressed
5. **Declarative**: The Lua config file is the single source of truth
6. **Simplicity**: Prebuilt binaries when available, human-readable store layout
7. **Cross-platform**: First-class support for Linux, macOS, and Windows

## Standard Lua Idioms

This is a core value that permeates the entire design:

```lua
-- sys.lua modules are plain Lua modules
local nginx = require("modules.services.nginx")
nginx.setup({ port = 8080 })

-- No magic. Just:
-- 1. require() returns a table
-- 2. setup() is a function call
-- 3. Options are plain tables
```

What this means in practice:

| Do                          | Don't                 |
| --------------------------- | --------------------- |
| `require()` + `setup()`     | Auto-evaluation magic |
| Plain tables for options    | Special DSL syntax    |
| Explicit function calls     | Implicit behavior     |
| Standard `for`/`if`/`while` | Custom control flow   |

If you know Lua, you know how to use sys.lua.

## The Two Primitives

Everything in sys.lua builds on two fundamental concepts:

```
Derivation (derive {})          Activation (activate {})
━━━━━━━━━━━━━━━━━━━━━━━━       ━━━━━━━━━━━━━━━━━━━━━━━━━━
Describes HOW to produce        Describes WHAT TO DO with
content for the store.          derivation output.

- Fetch from URL                - Add to PATH
- Clone git repo                - Create symlink
- Build from source             - Source in shell
- Generate config file          - Enable service

Output: immutable store object  Output: system side effects
```

All user-facing APIs (`file {}`, `env {}`, package `setup()`) internally create derivations and activations.

## Rust Surface Area

The Rust implementation is intentionally minimal, covering:

| Component       | Purpose                                 |
| --------------- | --------------------------------------- |
| **Derivations** | Hashing, realization, build context     |
| **Activations** | Execution of system side effects        |
| **Store**       | Content-addressed storage, immutability |
| **Lua parsing** | Config evaluation via mlua              |
| **Snapshots**   | History and rollback                    |

## Terminology

| Term             | Definition                                                       |
| ---------------- | ---------------------------------------------------------------- |
| **Derivation**   | Immutable description of how to produce store content            |
| **Activation**   | Description of what to do with derivation output                 |
| **Store**        | Global, immutable location for package content (`/syslua/store`) |
| **Store Object** | Content-addressed directory in `store/obj/<hash>/`               |
| **Manifest**     | Intermediate representation from evaluating Lua config           |
| **Snapshot**     | Point-in-time capture of derivations + activations               |
| **Input**        | Declared source of packages (GitHub repo, local path, Git URL)   |

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  User Config (init.lua)                  │
│  - Declares packages, files, env vars, services         │
│  - Uses Lua for logic and composition                   │
└───────────────────────────┬─────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                Evaluation & Resolution                   │
│  - Parse Lua → Manifest                                 │
│  - Resolve inputs from lock file                        │
│  - Priority-based conflict resolution                   │
└───────────────────────────┬─────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                  DAG Construction                        │
│  - Build execution graph from manifest                  │
│  - Topological sort, cycle detection                    │
└───────────────────────────┬─────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                 Parallel Execution                       │
│  - Realize derivations → store objects                  │
│  - Execute activations → system side effects            │
│  - Atomic: all-or-nothing with rollback                 │
└───────────────────────────┬─────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                   Immutable Store                        │
│  obj/<name>-<hash>/   Content-addressed objects         │
│  pkg/<name>/<ver>/    Human-readable symlinks           │
└─────────────────────────────────────────────────────────┘
```

## Document Index

This architecture is documented across focused files:

| Document                                 | Content                                       |
| ---------------------------------------- | --------------------------------------------- |
| [01-derivations.md](./01-derivations.md) | Derivation system, context API, hashing       |
| [02-activations.md](./02-activations.md) | Activation types, execution, examples         |
| [03-store.md](./03-store.md)             | Store layout, realization, immutability       |
| [04-lua-api.md](./04-lua-api.md)         | Lua API layers, globals, type definitions     |
| [05-snapshots.md](./05-snapshots.md)     | Snapshot design, rollback, garbage collection |
| [06-inputs.md](./06-inputs.md)           | Input sources, registry, lock files           |
| [07-modules.md](./07-modules.md)         | Module system, auto-evaluation                |
| [08-apply-flow.md](./08-apply-flow.md)   | Apply flow, DAG execution, atomicity          |
| [09-platform.md](./09-platform.md)       | Platform-specific: services, env, paths       |
| [10-crates.md](./10-crates.md)           | Crate structure and Rust dependencies         |

## Key Design Decisions

### Why Derivations + Activations?

Separating build (derivation) from deployment (activation) provides:

- **Better caching**: Same content with different targets = one derivation, multiple activations
- **Cleaner rollback**: Derivations are immutable; only activations change
- **Composability**: Multiple activations can reference the same derivation
- **Clear semantics**: Build logic stays pure; side effects are explicit

### Why Content-Addressed Storage?

- **Deduplication**: Same content = same hash = stored once
- **Reproducibility**: Hash guarantees identical content
- **Safe rollback**: Old versions remain in store until GC
- **Parallel safety**: No conflicts from concurrent operations

### Why Lua?

- **Familiar syntax**: Easy to read and write
- **Powerful**: First-class functions, tables for configuration
- **Safe**: No arbitrary system access from config
- **Embeddable**: mlua provides excellent Rust integration
