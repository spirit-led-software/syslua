---
type: feature
priority: medium
created: 2025-12-31T05:30:00Z
status: reviewed
tags: [cli, snapshot, management, ux]
keywords: [snapshot, SnapshotStore, SnapshotIndex, SnapshotMetadata, delete, list, tag, humantime, clap, subcommand]
patterns: [CLI subcommand structure, snapshot storage, index manipulation, time-based filtering]
---

# FEATURE: Snapshot Management Subcommand (`sys snapshot`)

## Description

Add a `sys snapshot` subcommand with `list`, `show`, `delete`, `tag`, and `untag` operations to allow users to manage snapshots directly. This enables users to view existing snapshots, inspect their contents, delete old or specific snapshots, and add human-readable tags for easier identification.

## Context

Currently, snapshots are created automatically during `sys apply` and can be rolled back via `sys rollback`, but there's no way to:
- List all existing snapshots
- View details of a specific snapshot
- Delete old/unwanted snapshots to reclaim space
- Add human-readable names/tags to snapshots

The existing `sys gc` command handles build/input garbage collection but not snapshot pruning. Users need explicit control over snapshot lifecycle management.

## Requirements

### Functional Requirements

#### `sys snapshot list`
- List all snapshots in reverse chronological order (newest first)
- Default output: bare minimum (ID, timestamp, current marker)
- `--verbose`: Show additional details (config path, build count, bind count, tags)
- `-o json`: Output full list as JSON

#### `sys snapshot show <id>`
- Display details of a specific snapshot
- Default output: ID, timestamp, config path, build/bind counts, tags
- `--verbose`: Include list of build names/versions and bind targets
- `-o json`: Dump full manifest as JSON

#### `sys snapshot delete <id>...`
- Delete one or more snapshots by explicit ID
- **Cannot delete the current snapshot** - error with message to use `sys destroy` first
- `--older-than <duration>`: Delete all snapshots older than the specified duration
  - Duration format via `humantime` crate: `7d`, `24h`, `2w`, `30d 12h`, etc.
  - Still requires explicit confirmation (or `--force`)
- `--dry-run`: Preview what would be deleted without actually deleting
- `--force`: Skip confirmation prompt
- Default behavior (no `--force`): Require interactive confirmation showing what will be deleted

#### `sys snapshot tag <id> <name>`
- Add a human-readable tag/name to a snapshot
- Tags are stored in `SnapshotMetadata` in the index
- Tags do not need to be unique (multiple snapshots can share a tag)

#### `sys snapshot untag <id>`
- Remove the tag from a snapshot

### Non-Functional Requirements

- Follow existing CLI patterns (see `gc.rs`, `info.rs` for reference)
- Support both human-readable and JSON output (`-o json`)
- Use `tracing` for logging at appropriate levels
- Cross-platform compatibility (use `syslua_lib::platform` abstractions)
- Acquire appropriate store lock for write operations (delete, tag, untag)

## Current State

- `SnapshotStore` exists with `list()`, `delete_snapshot()`, `load_snapshot()` methods
- `SnapshotMetadata` contains: `id`, `created_at`, `config_path`, `build_count`, `bind_count`
- `SnapshotIndex` tracks all snapshots and current pointer
- No CLI command exposes these operations
- No tag/name field exists in `SnapshotMetadata`

## Desired State

- New `sys snapshot` subcommand with `list`, `show`, `delete`, `tag`, `untag` operations
- `SnapshotMetadata` extended with `tag: Option<String>` field
- Users can manage snapshot lifecycle explicitly
- Destructive operations (delete) require confirmation by default

## Research Context

### Keywords to Search
- `SnapshotStore` - Core storage implementation
- `SnapshotIndex` - Index management, current pointer
- `SnapshotMetadata` - Metadata structure to extend with tags
- `cmd_gc` - Reference for CLI command structure with dry-run
- `clap` - CLI argument parsing patterns
- `humantime` - Duration parsing library

### Patterns to Investigate
- CLI subcommand structure in `crates/cli/src/cmd/`
- Output formatting patterns (`OutputFormat`, `print_json`, `print_stat`)
- Store locking patterns (`StoreLock::acquire`)
- Confirmation prompt patterns (may need to add)

### Key Decisions Made
- Use `humantime` crate for duration parsing
- Tags stored in `SnapshotMetadata` in index (not separate file)
- Tags are not unique across snapshots
- Cannot delete current snapshot (must `sys destroy` first)
- Only `--older-than` supported (no `--newer-than` for simplicity/safety)
- Explicit IDs required for delete (no glob/pattern matching)
- List shows newest first (reverse chronological)
- Confirmation required by default for delete operations

## Success Criteria

### Automated Verification
- [ ] `cargo build -p syslua-cli` succeeds
- [ ] `cargo test -p syslua-cli` passes
- [ ] `cargo test -p syslua-lib` passes (for SnapshotMetadata changes)
- [ ] `cargo clippy --all-targets --all-features` passes
- [ ] `cargo fmt --check` passes

### Manual Verification
- [ ] `sys snapshot list` shows all snapshots (newest first)
- [ ] `sys snapshot list --verbose` shows extended details
- [ ] `sys snapshot list -o json` outputs valid JSON
- [ ] `sys snapshot show <id>` displays snapshot details
- [ ] `sys snapshot show <id> --verbose` shows builds/binds
- [ ] `sys snapshot show <id> -o json` dumps manifest
- [ ] `sys snapshot delete <id>` prompts for confirmation
- [ ] `sys snapshot delete <id> --force` skips confirmation
- [ ] `sys snapshot delete <id> --dry-run` shows preview without deleting
- [ ] `sys snapshot delete --older-than 7d` deletes old snapshots with confirmation
- [ ] `sys snapshot delete <current-id>` errors with helpful message
- [ ] `sys snapshot tag <id> "name"` adds tag
- [ ] `sys snapshot untag <id>` removes tag
- [ ] Tags appear in `list` and `show` output

## Implementation Notes

### Files to Modify/Create

**New files:**
- `crates/cli/src/cmd/snapshot.rs` - Main command implementation

**Modify:**
- `crates/cli/src/cmd/mod.rs` - Add `snapshot` module and export
- `crates/cli/src/main.rs` - Add `snapshot` subcommand to clap
- `crates/lib/src/snapshot/types.rs` - Add `tag: Option<String>` to `SnapshotMetadata`
- `Cargo.toml` (cli) - Add `humantime` dependency

### Subcommand Structure

```rust
#[derive(Subcommand)]
enum SnapshotCommand {
    /// List all snapshots
    List {
        #[arg(short, long)]
        verbose: bool,
        #[arg(short = 'o', long, value_enum)]
        output: Option<OutputFormat>,
    },
    /// Show details of a specific snapshot
    Show {
        id: String,
        #[arg(short, long)]
        verbose: bool,
        #[arg(short = 'o', long, value_enum)]
        output: Option<OutputFormat>,
    },
    /// Delete snapshots
    Delete {
        /// Snapshot IDs to delete
        ids: Vec<String>,
        /// Delete snapshots older than this duration
        #[arg(long, value_parser = humantime::parse_duration)]
        older_than: Option<Duration>,
        /// Preview without deleting
        #[arg(long)]
        dry_run: bool,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Add a tag to a snapshot
    Tag {
        id: String,
        name: String,
    },
    /// Remove tag from a snapshot
    Untag {
        id: String,
    },
}
```

## Related Information

- [Architecture: Snapshots](../docs/architecture/05-snapshots.md)
- Existing `sys gc` command for reference patterns
- `sys rollback` for snapshot restoration
- `sys destroy` for removing current snapshot state

## Notes

- Consider adding integration tests similar to `gc_tests.rs`
- The confirmation prompt pattern may need to be extracted to a shared utility if not already present
- Ensure proper error messages when attempting to delete current snapshot
