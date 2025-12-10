# Snapshots

> Part of the [sys.lua Architecture](./00-overview.md) documentation.

This document covers the snapshot design, rollback algorithm, and garbage collection.

## Core Principle

**Derivations are immutable; activations are what change between snapshots.**

Snapshots capture system state using the **derivations + activations** model. A snapshot is simply a list of derivation hashes plus the activations that make them visible. This unified model eliminates the need for separate snapshot types for files, packages, and environment variables.

When you rollback, the derivations (content in the store) don't change - they're already there, cached by their content hash. What changes is which activations are active: which symlinks exist, which directories are in PATH, which services are enabled.

## Snapshot Structure

```rust
/// A snapshot captures system state as derivation hashes + activations.
pub struct Snapshot {
    /// Unique identifier (timestamp-based)
    pub id: String,

    /// Unix timestamp when the snapshot was created
    pub created_at: u64,

    /// Human-readable description
    pub description: String,

    /// Path to the configuration file that produced this state
    pub config_path: Option<PathBuf>,

    /// Hashes of all derivations in this snapshot
    /// These are just strings - the actual derivations live in store/obj/
    pub derivations: Vec<String>,

    /// Activations that make derivation outputs visible
    pub activations: Vec<Activation>,
}

/// An activation describes what to do with a derivation output.
pub struct Activation {
    /// The derivation hash this activation references
    pub derivation_hash: String,

    /// Which output to use (usually "out")
    pub output: String,

    /// The action to perform
    pub action: ActivationAction,
}

pub enum ActivationAction {
    /// Create a symlink to the derivation output
    Symlink {
        target: PathBuf,
        subpath: Option<String>,
        mutable: bool,
    },

    /// Add derivation's bin directory to PATH
    AddToPath {
        bin_subdir: Option<String>,
    },

    /// Source a script in shell init
    SourceInShell {
        shells: Vec<Shell>,
        script_subpath: String,
    },

    /// Manage a system service
    Service {
        service_type: ServiceType,
        enable: bool,
    },
}
```

## Storage Layout

```
~/.local/share/syslua/
├── snapshots/
│   ├── metadata.json           # Index of all snapshots
│   ├── <snapshot_id>.json      # Individual snapshot data
│   └── ...
└── store/
    └── obj/                    # Derivation outputs (immutable, content-addressed)
        ├── ripgrep-15.1.0-abc123/
        ├── file-gitconfig-def456/
        └── env-editor-ghi789/
```

### Metadata Index

```json
{
  "version": 1,
  "snapshots": [
    {
      "id": "1765208363188",
      "created_at": 1733667300,
      "description": "After successful apply",
      "derivation_count": 5,
      "activation_count": 8
    }
  ],
  "current": "1765208363188"
}
```

### Individual Snapshot

```json
{
  "id": "1765208363188",
  "created_at": 1733667300,
  "description": "After successful apply",
  "config_path": "/home/ian/.config/syslua/init.lua",

  "derivations": [
    "abc123def456789...",
    "def456abc123789...",
    "ghi789def456123..."
  ],

  "activations": [
    {
      "derivation_hash": "abc123def456789...",
      "output": "out",
      "action": { "AddToPath": { "bin_subdir": null } }
    },
    {
      "derivation_hash": "def456abc123789...",
      "output": "out",
      "action": {
        "Symlink": {
          "target": "/home/ian/.gitconfig",
          "subpath": "/content",
          "mutable": false
        }
      }
    },
    {
      "derivation_hash": "ghi789def456123...",
      "output": "out",
      "action": {
        "SourceInShell": {
          "shells": ["Bash", "Zsh"],
          "script_subpath": "env.sh"
        }
      }
    }
  ]
}
```

## Why This Model is Better

| Aspect                    | Old Model (separate types)      | New Model (derivations + activations) |
| ------------------------- | ------------------------------- | ------------------------------------- |
| **Type proliferation**    | SnapshotFile, SnapshotEnv, etc. | Just Activation with variants         |
| **Adding new features**   | New struct for each feature     | New ActivationAction variant          |
| **Diff clarity**          | Compare heterogeneous lists     | Compare derivation sets + activations |
| **GC integration**        | Must track refs from each type  | Derivation hashes are the refs        |
| **Rollback logic**        | Different logic per type        | Uniform: deactivate/activate          |
| **Content deduplication** | Per-type deduplication          | Single derivation store               |

## What Gets Captured

Everything is captured through derivations and activations:

| User Action                       | Derivation              | Activation                                   |
| --------------------------------- | ----------------------- | -------------------------------------------- |
| `require("pkgs.cli.ripgrep").setup()` | Package build/fetch     | `AddToPath { bin_subdir: None }`             |
| `file { path, src }`              | Content copy to store   | `Symlink { target, subpath: "/content" }`    |
| `file { mutable }`                | Metadata (link info)    | `Symlink { target, mutable: true }`          |
| `env { EDITOR }`                  | Shell fragments         | `SourceInShell { shells, script: "env.sh" }` |
| `require("modules.services.nginx").setup()` | Service unit derivation | `Service { type: Systemd, enable: true }`    |

## Rollback

Rollback is straightforward with the derivations + activations model:

```bash
$ sys rollback                    # Rollback to previous snapshot
$ sys rollback <snapshot_id>      # Rollback to specific snapshot
$ sys rollback --dry-run          # Preview what would change
```

**Key insight**: Derivations don't need to be "rolled back" - they're immutable in the store. Only activations change.

## Rollback Algorithm

```
ROLLBACK_TO_SNAPSHOT(target_snapshot_id, dry_run=false):
    target = LOAD_SNAPSHOT(target_snapshot_id)
    IF target IS NULL:
        ERROR "Snapshot '{target_snapshot_id}' not found"

    current = GET_CURRENT_SNAPSHOT()

    // Phase 1: Compute activation diff
    activations_to_remove = current.activations - target.activations
    activations_to_add = target.activations - current.activations

    // Phase 2: Display changes
    PRINT_ROLLBACK_PLAN(activations_to_remove, activations_to_add)

    IF dry_run:
        RETURN

    IF NOT CONFIRM("Proceed with rollback?"):
        RETURN

    // Phase 3: Create pre-rollback snapshot
    pre_rollback = CREATE_SNAPSHOT("Before rollback to " + target_snapshot_id)

    // Phase 4: Execute rollback (atomic)
    TRY:
        // Deactivate activations not in target
        FOR EACH activation IN activations_to_remove:
            DEACTIVATE(activation)

        // Activate activations in target
        FOR EACH activation IN activations_to_add:
            drv_output = STORE.get_output(activation.derivation_hash)
            IF drv_output IS NULL:
                ERROR "Derivation {activation.derivation_hash} not found in store"
            ACTIVATE(activation, drv_output)

        // Update current pointer
        SET_CURRENT_SNAPSHOT(target_snapshot_id)
        PRINT "Rollback successful"

    CATCH error:
        ERROR "Rollback failed: {error}"
        PRINT "Restoring pre-rollback state..."
        ROLLBACK_TO_SNAPSHOT(pre_rollback.id, dry_run=false)
        ERROR "Rollback aborted. System restored to pre-rollback state."
```

### Deactivation Logic

```
DEACTIVATE(activation):
    SWITCH activation.action:
        CASE Symlink { target, ... }:
            REMOVE_SYMLINK(target)

        CASE AddToPath { ... }:
            // Will be regenerated when new activations are applied
            PASS

        CASE SourceInShell { ... }:
            // Will be regenerated when new activations are applied
            PASS

        CASE Service { service_type, ... }:
            STOP_SERVICE(service_type, activation.derivation_hash)
            DISABLE_SERVICE(service_type, activation.derivation_hash)
```

### Activation Logic

```
ACTIVATE(activation, drv_output):
    SWITCH activation.action:
        CASE Symlink { target, subpath, mutable }:
            source = drv_output
            IF subpath:
                source = drv_output + subpath

            IF mutable:
                original_source = READ_MUTABLE_SOURCE(drv_output)
                CREATE_SYMLINK(original_source, target)
            ELSE:
                CREATE_SYMLINK(source, target)

        CASE AddToPath { bin_subdir }:
            bin_path = drv_output + "/" + (bin_subdir OR "bin")
            ADD_TO_PATH_ACTIVATION(bin_path)

        CASE SourceInShell { shells, script_subpath }:
            script_path = drv_output + "/" + script_subpath
            FOR EACH shell IN shells:
                ADD_TO_SHELL_INIT(shell, script_path)

        CASE Service { service_type, enable }:
            IF enable:
                INSTALL_SERVICE(service_type, drv_output)
                ENABLE_SERVICE(service_type, drv_output)
                START_SERVICE(service_type, drv_output)
```

## Garbage Collection

The GC algorithm is simplified with the derivations + activations model:

```
GARBAGE_COLLECT():
    // Collect all derivation hashes referenced by any snapshot
    referenced_hashes = SET()
    FOR EACH snapshot IN ALL_SNAPSHOTS():
        referenced_hashes.add_all(snapshot.derivations)

    // Remove unreferenced objects from store
    FOR EACH obj_dir IN store/obj/*:
        hash = EXTRACT_HASH(obj_dir)
        IF hash NOT IN referenced_hashes:
            REMOVE_IMMUTABILITY(obj_dir)
            DELETE(obj_dir)
```

### GC with Locking

To prevent race conditions, GC uses a global lock:

```
GC_COLLECT():
    lock = ACQUIRE_STORE_LOCK(exclusive=true, timeout=30s)
    IF lock IS NULL:
        ERROR "Could not acquire store lock. Another sys.lua operation may be running."

    TRY:
        // Phase 1: Find all roots
        roots = SET()

        // Add all package symlinks
        FOR EACH symlink IN GLOB("store/pkg/**/*"):
            IF IS_SYMLINK(symlink):
                target = READ_LINK(symlink)
                hash = EXTRACT_HASH_FROM_PATH(target)
                roots.add(hash)

        // Add all snapshots
        FOR EACH snapshot IN LOAD_ALL_SNAPSHOTS():
            FOR EACH drv_hash IN snapshot.derivations:
                roots.add(drv_hash)

        // Phase 2: Find unreferenced objects
        unreferenced = []
        FOR EACH obj_path IN GLOB("store/obj/*"):
            hash = EXTRACT_HASH(obj_path)
            IF hash NOT IN roots:
                unreferenced.append({ hash, path: obj_path })

        // Phase 3: Remove unreferenced objects
        total_size = 0
        FOR EACH { hash, path } IN unreferenced:
            size = GET_DIRECTORY_SIZE(path)
            total_size += size
            MAKE_MUTABLE(path)
            REMOVE_DIRECTORY(path)

        PRINT "Removed {unreferenced.length} objects, freed {total_size} bytes"

    FINALLY:
        RELEASE_STORE_LOCK(lock)
```

### Concurrent Operation Protection

| Operation    | Lock Type     | Blocks GC? | Blocked by GC? |
| ------------ | ------------- | ---------- | -------------- |
| `sys apply`  | Exclusive     | Yes        | Yes            |
| `sys gc`     | Exclusive     | N/A        | Yes (by apply) |
| `sys plan`   | Shared (read) | No         | No             |
| `sys status` | Shared (read) | No         | No             |
| `sys shell`  | Shared (read) | No         | No             |

### GC and Snapshots

Snapshots protect their referenced objects from GC:

```bash
$ sys apply init.lua           # Installs ripgrep@15.1.0 (creates snapshot 1)
$ # Edit sys.lua to remove ripgrep
$ sys apply init.lua           # Removes ripgrep symlink (creates snapshot 2)
$ sys gc                       # Does NOT delete ripgrep object (snapshot 1 references it)
$ sys rollback <snapshot 1>    # Can still rollback (object exists)
$ sys gc --delete-old-snapshots --keep 5  # Delete old snapshots
$ sys gc                       # NOW ripgrep object can be deleted
```

## Comparing Snapshots

With derivations + activations, comparing snapshots is clear:

```bash
$ sys diff <snapshot_a> <snapshot_b>

Derivation changes:
  + ripgrep-16.0.0-newhhash  (new version)
  - ripgrep-15.1.0-oldhash   (removed)
  = neovim-0.10.0-abc123     (unchanged)

Activation changes:
  ~ Symlink ~/.gitconfig     (different derivation: def456 → ghi789)
  + Service postgresql       (added)
  - AddToPath /old/tool/bin  (removed)
```

This clear separation makes it easy to understand what changed between configurations.

## See Also

- [Store](./03-store.md) - Where derivation outputs live
- [Derivations](./01-derivations.md) - How derivations work
- [Activations](./02-activations.md) - How activations work
- [Apply Flow](./08-apply-flow.md) - How snapshots are created during apply
