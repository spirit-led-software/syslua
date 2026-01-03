---
date: 2025-12-31T07:16:57-05:00
git_commit: c9f929568973d5a45f2c6f12d551ea61fbe85855
branch: feat/gc-command
repository: syslua
topic: "Snapshot Management Subcommand Implementation"
tags: [research, codebase, cli, snapshot, clap, confirmation-prompts]
last_updated: 2025-12-31
---

## Ticket Synopsis

Add a `sys snapshot` subcommand with `list`, `show`, `delete`, `tag`, and `untag` operations. Users need to manage snapshot lifecycle explicitly - viewing existing snapshots, deleting old ones, and adding human-readable tags. The delete operation requires confirmation by default (with `--force` to skip), supports `--older-than` duration filtering via humantime, and cannot delete the current snapshot.

## Summary

The implementation requires:
1. **New CLI module** (`cmd/snapshot.rs`) with nested clap subcommands
2. **SnapshotMetadata extension** to add `tag: Option<String>` field with serde default for backwards compatibility
3. **New error variant** `SnapshotError::CannotDeleteCurrent` to prevent current snapshot deletion
4. **Confirmation prompt system** (new to codebase) using stdlib TTY detection
5. **humantime dependency** for duration parsing (not currently in codebase)

Key patterns exist for dry-run, JSON output, and store locking that can be reused. No confirmation prompt pattern exists - it must be created.

## Detailed Findings

### CLI Command Structure

The existing CLI follows a consistent pattern:
- Commands enum at `main.rs:77-161` with dispatch at `main.rs:210-232`
- Pattern: `Commands::X { args } => cmd_x(args)`
- Module organization at `cmd/mod.rs:14-32`
- OutputFormat enum at `output.rs:13-23` with Text/Json variants and `is_json()` method
- Global flags: `--log-level`, `--log-format`, `--color`

**For nested subcommands**, use:
```rust
Commands::Snapshot {
    #[command(subcommand)]
    command: SnapshotCommand,
}
```

Then dispatch with two-level match:
```rust
Commands::Snapshot { command } => match command {
    SnapshotCommand::List { verbose, output } => cmd_snapshot_list(verbose, output),
    // ...
}
```

### Snapshot Storage System

**Files and locations:**
- `SnapshotStore` at `storage.rs:31-34`
- `SnapshotMetadata` at `types.rs:72-88` 
- `SnapshotIndex` at `types.rs:111-121`
- Storage at `~/.local/share/syslua/snapshots/` with `index.json` + `{id}.json` files

**Key methods:**
- `SnapshotStore::list()` at line 215 - returns all metadata
- `SnapshotStore::delete_snapshot(id)` at line 224 - removes file + updates index
- `SnapshotStore::current_id()` at line 108 - returns current snapshot ID
- `SnapshotStore::load_snapshot(id)` at line 125 - loads full snapshot

**Current delete behavior (needs modification):**
- Deletes `{id}.json` file
- Updates index via `index.remove(id)`
- If deleted snapshot was current, **clears current pointer** (this will change to error)

### SnapshotMetadata Tag Extension

**Required changes:**

1. Add field with serde default (types.rs:72-88):
```rust
pub struct SnapshotMetadata {
    pub id: String,
    pub created_at: u64,
    pub config_path: Option<PathBuf>,
    #[serde(default)]
    pub tag: Option<String>,
    pub build_count: usize,
    pub bind_count: usize,
}
```

2. Update `Snapshot::to_metadata()` (types.rs:57-65) to include `tag: None`

3. Add methods for tag management:
```rust
// In SnapshotIndex
pub fn update_tag(&mut self, id: &str, tag: Option<String>) -> Result<(), SnapshotError>

// In SnapshotStore
pub fn update_snapshot_tag(&self, id: &str, tag: Option<String>) -> Result<(), SnapshotError>
```

4. Update all test struct literals to include `tag: None` (types.rs tests around lines 283-358, storage.rs test around line 494-500)

**Backwards compatibility:** `#[serde(default)]` is REQUIRED - without it, old index.json files missing the "tag" field will fail to deserialize. `Option<String>` alone is NOT sufficient.

### Current Snapshot Protection

**Current state:** No protection exists. `delete_snapshot()` happily deletes current and clears the pointer.

**Required changes:**

1. Add error variant (types.rs):
```rust
#[error("cannot delete current snapshot: {0}")]
CannotDeleteCurrent(String),
```

2. Add guard in `delete_snapshot()` (storage.rs):
```rust
pub fn delete_snapshot(&self, id: &str) -> Result<(), SnapshotError> {
    let mut index = self.load_index()?;

    if index.current.as_deref() == Some(id) {
        return Err(SnapshotError::CannotDeleteCurrent(id.to_string()));
    }

    // ... rest of deletion logic
}
```

### Confirmation Prompts (New Pattern)

**No confirmation mechanism exists in codebase.** No dialoguer/inquire dependencies, no stdin reading, no --force flags.

**Recommended approach:** Use stdlib TTY detection (`std::io::IsTerminal`, Rust 1.70+) + custom y/N reader.

**Implementation pattern:**
```rust
// crates/cli/src/prompts.rs
use std::io::{self, IsTerminal, Write};

pub fn confirm_delete(message: &str, force: bool) -> Result<bool> {
    if force {
        return Ok(true);
    }

    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("Non-interactive terminal. Pass --force to proceed.");
    }

    write!(io::stderr(), "{} [y/N] ", message)?;
    io::stderr().flush()?;

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}
```

**Key decisions:**
- Print prompts to **stderr** so stdout can be piped
- Default to No (pressing Enter = cancel)
- In non-interactive mode: **error requiring --force** (safer than silent default)
- No external dependencies needed

### Dry-Run Pattern

Existing pattern in gc.rs:
- Flag: `#[arg(long)] dry_run: bool`
- Collect candidates regardless of dry-run
- Conditionally skip actual deletion: `if dry_run { record_stats } else { fs::remove_... }`
- Output: `if dry_run { print_info("Dry run - no changes made") }`

### JSON Output Pattern

- OutputFormat enum at output.rs:12-23 with `is_json()` method
- print_json helper at output.rs:115-119 uses `serde_json::to_string_pretty`
- Pattern: `if output.is_json() { print_json(&result)? } else { /* text */ }`
- Flag: `#[arg(short = 'o', long, value_enum, default_value = "text")] output: OutputFormat`

### Store Locking

- StoreLock at store_lock.rs:62-65
- `StoreLock::acquire(LockMode::Exclusive, "command_name")?` at function start
- Pattern from apply.rs:197, gc.rs:13
- Mutating commands (delete, tag, untag) need exclusive lock

### humantime Integration

**Not currently in codebase** - must add dependency.

Add to workspace Cargo.toml and cli Cargo.toml:
```toml
humantime = "2.1"
```

Integration with clap:
```rust
#[arg(long, value_parser = humantime::parse_duration)]
older_than: Option<Duration>
```

Supports: `30s`, `5min`, `2h`, `7days`, `"15days 2min 2s"`

## Code References

- `crates/cli/src/main.rs:77-161` - Commands enum definition
- `crates/cli/src/main.rs:210-232` - Command dispatch
- `crates/cli/src/cmd/mod.rs:14-32` - Module exports
- `crates/cli/src/cmd/gc.rs:10-33` - GC command handler with dry-run
- `crates/cli/src/output.rs:12-23` - OutputFormat enum
- `crates/cli/src/output.rs:115-119` - print_json helper
- `crates/lib/src/snapshot/types.rs:72-88` - SnapshotMetadata struct
- `crates/lib/src/snapshot/types.rs:111-121` - SnapshotIndex struct
- `crates/lib/src/snapshot/storage.rs:31-34` - SnapshotStore struct
- `crates/lib/src/snapshot/storage.rs:215` - list() method
- `crates/lib/src/snapshot/storage.rs:224` - delete_snapshot() method
- `crates/lib/src/store_lock.rs:62-65` - StoreLock struct
- `crates/lib/src/store_lock.rs:83-111` - acquire() method

## Architecture Insights

1. **CLI pattern consistency**: All commands follow `cmd_x(args)` pattern with OutputFormat support
2. **Atomic writes**: Snapshot storage uses temp file + rename for atomic updates
3. **RAII locking**: StoreLock released on drop, held for entire operation
4. **Serde defaults**: Critical for backwards compatibility when extending types
5. **Nested subcommands**: Use `#[command(subcommand)]` on enum variant field

## Historical Context (from thoughts/)

- `docs/architecture/05-snapshots.md` - Snapshot design, rollback algorithm, GC spec (mark-sweep), locking spec
- `thoughts/research/2025-12-31_gc_command.md` - Full codebase analysis for GC command, similar patterns apply
- `thoughts/plans/gc_command_implementation.md` - 5-phase implementation approach for GC (reference for structuring snapshot implementation)
- `thoughts/tickets/feature_gc_command.md` - GC ticket structure (reference)

## Related Research

- `thoughts/research/2025-12-31_gc_command.md` - Similar implementation patterns for store operations

## Open Questions

1. **Test structure**: Should integration tests go in `cli/tests/integration/snapshot_tests.rs` (like gc_tests.rs)?
2. **Prompts module location**: Create `crates/cli/src/prompts.rs` or add to `output.rs`?
3. **Tag storage on Snapshot struct**: Currently only SnapshotMetadata will have tag - should Snapshot struct also get it for consistency?
4. **Error handling in delete loop**: If deleting multiple snapshots and one fails, should it:
   - Stop immediately and report error?
   - Continue and report all failures at end?
   - Delete what it can and report partial success?
