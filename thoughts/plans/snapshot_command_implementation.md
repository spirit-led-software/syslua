# Snapshot Command Implementation Plan

## Overview

Implement `sys snapshot` subcommand with `list`, `show`, `delete`, `tag`, and `untag` operations to allow users to manage snapshots directly. This enables explicit snapshot lifecycle management including viewing, inspecting, deleting, and tagging snapshots.

## Current State Analysis

- `SnapshotStore` exists with `list()`, `delete_snapshot()`, `load_snapshot()` methods
- `SnapshotMetadata` contains: `id`, `created_at`, `config_path`, `build_count`, `bind_count` - **no tag field**
- `SnapshotIndex` tracks all snapshots and current pointer
- No CLI command exposes snapshot management operations
- No confirmation prompt pattern exists in the codebase
- `humantime` dependency not present

### Key Discoveries:

- `SnapshotMetadata` at types.rs:72-88 needs `tag: Option<String>` field
- `SnapshotStore::list()` returns snapshots in chronological order (oldest first) - need to reverse for display
- `SnapshotStore::delete_snapshot()` can delete current snapshot - protection must be at CLI layer, not lib layer (to allow `sys destroy` to work)
- GC command pattern at gc.rs shows dry-run, output format, store locking patterns
- Commands enum uses `#[command(subcommand)]` for nested subcommands

## Desired End State

After this plan is complete:

1. `sys snapshot list` displays all snapshots (newest first) with optional verbose/JSON output
2. `sys snapshot show <id>` displays snapshot details with optional verbose/JSON output
3. `sys snapshot delete` removes snapshots with confirmation (bypassed via `--force`), supports `--older-than` duration filtering, prevents deletion of current snapshot with helpful error
4. `sys snapshot tag <id> <name>` adds human-readable tag to snapshot
5. `sys snapshot untag <id>` removes tag from snapshot
6. Tags appear in list/show output
7. All operations follow existing CLI patterns (output format, store locking, tracing)

### Verification:

- All tests pass: `cargo test`
- Linting passes: `cargo clippy --all-targets --all-features`
- Formatting passes: `cargo fmt --check`
- Manual testing of all subcommands confirms expected behavior

## What We're NOT Doing

- Pattern/glob matching for snapshot IDs (explicit IDs only)
- `--newer-than` filtering (only `--older-than` for safety)
- Unique tag enforcement (multiple snapshots can share a tag)
- Tag-based deletion (delete by ID only)
- Any changes to `sys destroy` behavior
- Protection at lib layer (protection is CLI-only)

## Implementation Approach

Five phases, each building on the previous:

1. **Library Extensions** - Extend SnapshotMetadata with tag, add tag management methods
2. **CLI Infrastructure** - Add humantime dependency, create prompts module
3. **Command Implementation** - Implement all snapshot subcommands
4. **CLI Integration** - Wire up command in main.rs
5. **Testing** - Integration tests for all operations

---

## Phase 1: Library Extensions

### Overview

Extend `SnapshotMetadata` with a `tag` field and add methods for tag management. This phase modifies only library code.

### Changes Required:

#### 1. SnapshotMetadata Tag Field

**File**: `crates/lib/src/snapshot/types.rs`

Add `tag` field to `SnapshotMetadata` struct (around line 72-88):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotMetadata {
    pub id: String,
    pub created_at: u64,
    pub config_path: Option<PathBuf>,
    pub tag: Option<String>,
    pub build_count: usize,
    pub bind_count: usize,
}
```

#### 2. Update Snapshot::to_metadata()

**File**: `crates/lib/src/snapshot/types.rs`

Update `to_metadata()` method (around line 57-65) to include `tag: None`:

```rust
pub fn to_metadata(&self) -> SnapshotMetadata {
    SnapshotMetadata {
        id: self.id.clone(),
        created_at: self.created_at,
        config_path: self.config_path.clone(),
        tag: None,
        build_count: self.builds.len(),
        bind_count: self.binds.len(),
    }
}
```

#### 3. Add SnapshotIndex::update_tag() Method

**File**: `crates/lib/src/snapshot/types.rs`

Add method to `SnapshotIndex` impl block (after existing methods around line 150):

```rust
/// Update the tag for a snapshot in the index
pub fn update_tag(&mut self, id: &str, tag: Option<String>) -> Result<(), SnapshotError> {
    let metadata = self
        .snapshots
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| SnapshotError::NotFound(id.to_string()))?;
    metadata.tag = tag;
    Ok(())
}
```

#### 4. Add SnapshotStore::update_snapshot_tag() Method

**File**: `crates/lib/src/snapshot/storage.rs`

Add method to `SnapshotStore` impl block (after existing methods):

```rust
/// Update the tag for a snapshot
pub fn update_snapshot_tag(&self, id: &str, tag: Option<String>) -> Result<(), SnapshotError> {
    let mut index = self.load_index()?;
    index.update_tag(id, tag)?;
    self.save_index(&index)?;
    Ok(())
}
```

#### 5. Update Test Fixtures

**File**: `crates/lib/src/snapshot/types.rs`

Update all `SnapshotMetadata` literals in tests (around lines 283-365) to include `tag: None`:

```rust
SnapshotMetadata {
    id: "test-id".to_string(),
    created_at: 12345,
    config_path: Some(PathBuf::from("/test/path")),
    tag: None,  // Add this line
    build_count: 5,
    bind_count: 3,
}
```

**File**: `crates/lib/src/snapshot/storage.rs`

Update test fixtures (around lines 494-500) similarly.

### Success Criteria:

#### Automated Verification:

- [x] `cargo build -p syslua-lib` succeeds
- [x] `cargo test -p syslua-lib snapshot` passes
- [x] Existing index.json files (without tag field) will fail to deserialize (acceptable pre-1.0)

#### Manual Verification:

- [x] Create a snapshot, verify tag field defaults to None
- [x] Verify old snapshots still load after code changes

---

## Phase 2: CLI Infrastructure

### Overview

Add `humantime` dependency for duration parsing and create a `prompts` module for confirmation dialogs.

### Changes Required:

#### 1. Add humantime Dependency

**File**: `Cargo.toml` (workspace root)

Add to `[workspace.dependencies]`:

```toml
humantime = "2.1"
```

**File**: `crates/cli/Cargo.toml`

Add to `[dependencies]`:

```toml
humantime = { workspace = true }
```

#### 2. Create Prompts Module

**File**: `crates/cli/src/prompts.rs` (new file)

```rust
//! Interactive confirmation prompts for destructive operations.

use anyhow::{bail, Result};
use std::io::{self, IsTerminal, Write};

/// Prompt for confirmation before a destructive action.
///
/// Returns `Ok(true)` if confirmed, `Ok(false)` if declined.
/// Returns an error if not in an interactive terminal and `force` is false.
///
/// # Arguments
/// * `message` - The confirmation message to display
/// * `force` - If true, skip the prompt and return true
pub fn confirm(message: &str, force: bool) -> Result<bool> {
    if force {
        return Ok(true);
    }

    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("Cannot prompt for confirmation in non-interactive mode. Use --force to proceed.");
    }

    write!(io::stderr(), "{} [y/N] ", message)?;
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
```

#### 3. Export Prompts Module

**File**: `crates/cli/src/main.rs`

Add module declaration (near top with other mod declarations):

```rust
mod prompts;
```

### Success Criteria:

#### Automated Verification:

- [x] `cargo build -p syslua-cli` succeeds
- [x] `cargo test -p syslua-cli` passes

#### Manual Verification:

- [x] N/A (prompts tested via snapshot delete in Phase 3)

---

## Phase 3: Snapshot Command Implementation

### Overview

Create the `cmd/snapshot.rs` module implementing all snapshot subcommands.

### Changes Required:

#### 1. Create Snapshot Command Module

**File**: `crates/cli/src/cmd/snapshot.rs` (new file)

```rust
//! Snapshot management subcommand.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use clap::Subcommand;
use serde::Serialize;
use syslua_lib::{platform::SysluaPaths, snapshot::SnapshotStore, store_lock::{LockMode, StoreLock}};
use tracing::{debug, info};

use crate::output::{print_error, print_info, print_json, print_success, print_warning, OutputFormat};
use crate::prompts::confirm;

/// Snapshot management operations
#[derive(Subcommand, Debug)]
pub enum SnapshotCommand {
    /// List all snapshots
    List {
        /// Show additional details (config path, build/bind counts, tags)
        #[arg(short, long)]
        verbose: bool,

        /// Output format
        #[arg(short = 'o', long, value_enum, default_value = "text")]
        output: OutputFormat,
    },

    /// Show details of a specific snapshot
    Show {
        /// Snapshot ID to show
        id: String,

        /// Include list of builds and binds
        #[arg(short, long)]
        verbose: bool,

        /// Output format
        #[arg(short = 'o', long, value_enum, default_value = "text")]
        output: OutputFormat,
    },

    /// Delete snapshots
    Delete {
        /// Snapshot IDs to delete
        ids: Vec<String>,

        /// Delete snapshots older than this duration (e.g., "7d", "24h", "2w")
        #[arg(long, value_parser = humantime::parse_duration)]
        older_than: Option<Duration>,

        /// Preview what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,

        /// Output format
        #[arg(short = 'o', long, value_enum, default_value = "text")]
        output: OutputFormat,
    },

    /// Add a tag to a snapshot
    Tag {
        /// Snapshot ID to tag
        id: String,

        /// Tag name to apply
        name: String,
    },

    /// Remove tag from a snapshot
    Untag {
        /// Snapshot ID to untag
        id: String,
    },
}

/// Result of a delete operation for JSON output
#[derive(Debug, Serialize)]
struct DeleteResult {
    deleted: Vec<String>,
    failed: Vec<DeleteFailure>,
    skipped_current: Option<String>,
    dry_run: bool,
}

#[derive(Debug, Serialize)]
struct DeleteFailure {
    id: String,
    error: String,
}

/// Execute a snapshot subcommand
pub fn cmd_snapshot(command: SnapshotCommand) -> Result<()> {
    match command {
        SnapshotCommand::List { verbose, output } => cmd_list(verbose, output),
        SnapshotCommand::Show { id, verbose, output } => cmd_show(&id, verbose, output),
        SnapshotCommand::Delete {
            ids,
            older_than,
            dry_run,
            force,
            output,
        } => cmd_delete(ids, older_than, dry_run, force, output),
        SnapshotCommand::Tag { id, name } => cmd_tag(&id, &name),
        SnapshotCommand::Untag { id } => cmd_untag(&id),
    }
}

fn cmd_list(verbose: bool, output: OutputFormat) -> Result<()> {
    let paths = SysluaPaths::new()?;
    let store = SnapshotStore::new(paths.snapshots_dir());

    let mut snapshots = store.list()?;
    let current_id = store.current_id()?;

    // Reverse to show newest first
    snapshots.reverse();

    if output.is_json() {
        #[derive(Serialize)]
        struct ListOutput {
            snapshots: Vec<SnapshotListItem>,
            current: Option<String>,
        }

        #[derive(Serialize)]
        struct SnapshotListItem {
            id: String,
            created_at: u64,
            is_current: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            config_path: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            tag: Option<String>,
            build_count: usize,
            bind_count: usize,
        }

        let items: Vec<SnapshotListItem> = snapshots
            .iter()
            .map(|s| SnapshotListItem {
                id: s.id.clone(),
                created_at: s.created_at,
                is_current: current_id.as_ref() == Some(&s.id),
                config_path: s.config_path.as_ref().map(|p| p.display().to_string()),
                tag: s.tag.clone(),
                build_count: s.build_count,
                bind_count: s.bind_count,
            })
            .collect();

        print_json(&ListOutput {
            snapshots: items,
            current: current_id,
        })?;
    } else {
        if snapshots.is_empty() {
            print_info("No snapshots found");
            return Ok(());
        }

        for snapshot in &snapshots {
            let is_current = current_id.as_ref() == Some(&snapshot.id);
            let current_marker = if is_current { " (current)" } else { "" };
            let timestamp = format_timestamp(snapshot.created_at);

            if verbose {
                let tag_str = snapshot
                    .tag
                    .as_ref()
                    .map(|t| format!(" [{}]", t))
                    .unwrap_or_default();
                let config_str = snapshot
                    .config_path
                    .as_ref()
                    .map(|p| format!(" config={}", p.display()))
                    .unwrap_or_default();

                println!(
                    "{}{}{} - {}{} (builds: {}, binds: {})",
                    snapshot.id,
                    current_marker,
                    tag_str,
                    timestamp,
                    config_str,
                    snapshot.build_count,
                    snapshot.bind_count
                );
            } else {
                let tag_str = snapshot
                    .tag
                    .as_ref()
                    .map(|t| format!(" [{}]", t))
                    .unwrap_or_default();
                println!("{}{}{} - {}", snapshot.id, current_marker, tag_str, timestamp);
            }
        }

        print_info(&format!("{} snapshot(s) total", snapshots.len()));
    }

    Ok(())
}

fn cmd_show(id: &str, verbose: bool, output: OutputFormat) -> Result<()> {
    let paths = SysluaPaths::new()?;
    let store = SnapshotStore::new(paths.snapshots_dir());

    let snapshot = store.load_snapshot(id)?;
    let current_id = store.current_id()?;
    let is_current = current_id.as_ref() == Some(&snapshot.id);

    // Get metadata for tag (snapshot struct doesn't have tag, only metadata does)
    let metadata = store.list()?.into_iter().find(|m| m.id == id);
    let tag = metadata.and_then(|m| m.tag);

    if output.is_json() {
        #[derive(Serialize)]
        struct ShowOutput {
            id: String,
            created_at: u64,
            is_current: bool,
            config_path: Option<String>,
            tag: Option<String>,
            builds: Vec<BuildInfo>,
            binds: Vec<BindInfo>,
        }

        #[derive(Serialize)]
        struct BuildInfo {
            name: String,
            version: String,
            hash: String,
        }

        #[derive(Serialize)]
        struct BindInfo {
            target: String,
            source_hash: String,
        }

        let builds: Vec<BuildInfo> = snapshot
            .builds
            .iter()
            .map(|b| BuildInfo {
                name: b.name.clone(),
                version: b.version.clone(),
                hash: b.hash.0.clone(),
            })
            .collect();

        let binds: Vec<BindInfo> = snapshot
            .binds
            .iter()
            .map(|b| BindInfo {
                target: b.target.display().to_string(),
                source_hash: b.source_hash.0.clone(),
            })
            .collect();

        print_json(&ShowOutput {
            id: snapshot.id.clone(),
            created_at: snapshot.created_at,
            is_current,
            config_path: snapshot.config_path.as_ref().map(|p| p.display().to_string()),
            tag,
            builds,
            binds,
        })?;
    } else {
        let current_marker = if is_current { " (current)" } else { "" };
        let timestamp = format_timestamp(snapshot.created_at);
        let tag_str = tag
            .as_ref()
            .map(|t| format!(" [{}]", t))
            .unwrap_or_default();

        println!("Snapshot: {}{}{}", snapshot.id, current_marker, tag_str);
        println!("Created:  {}", timestamp);
        if let Some(config) = &snapshot.config_path {
            println!("Config:   {}", config.display());
        }
        println!("Builds:   {}", snapshot.builds.len());
        println!("Binds:    {}", snapshot.binds.len());

        if verbose {
            if !snapshot.builds.is_empty() {
                println!("\nBuilds:");
                for build in &snapshot.builds {
                    println!("  {} v{} ({})", build.name, build.version, &build.hash.0[..12]);
                }
            }

            if !snapshot.binds.is_empty() {
                println!("\nBinds:");
                for bind in &snapshot.binds {
                    println!("  {} -> {}", &bind.source_hash.0[..12], bind.target.display());
                }
            }
        }
    }

    Ok(())
}

fn cmd_delete(
    ids: Vec<String>,
    older_than: Option<Duration>,
    dry_run: bool,
    force: bool,
    output: OutputFormat,
) -> Result<()> {
    let paths = SysluaPaths::new()?;
    let store = SnapshotStore::new(paths.snapshots_dir());

    // Collect candidates
    let mut candidates: Vec<String> = ids;
    let current_id = store.current_id()?;

    // Add snapshots matching --older-than
    if let Some(duration) = older_than {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let cutoff = now.saturating_sub(duration.as_secs());

        let snapshots = store.list()?;
        for snapshot in snapshots {
            if snapshot.created_at < cutoff && !candidates.contains(&snapshot.id) {
                candidates.push(snapshot.id);
            }
        }
    }

    if candidates.is_empty() {
        if output.is_json() {
            print_json(&DeleteResult {
                deleted: vec![],
                failed: vec![],
                skipped_current: None,
                dry_run,
            })?;
        } else {
            print_info("No snapshots to delete");
        }
        return Ok(());
    }

    // Check for current snapshot in candidates
    let mut skipped_current: Option<String> = None;
    if let Some(ref current) = current_id {
        if candidates.contains(current) {
            skipped_current = Some(current.clone());
            candidates.retain(|id| id != current);
        }
    }

    if candidates.is_empty() {
        if output.is_json() {
            print_json(&DeleteResult {
                deleted: vec![],
                failed: vec![],
                skipped_current,
                dry_run,
            })?;
        } else {
            print_warning("Cannot delete the current snapshot. Use 'sys destroy' first.");
        }
        return Ok(());
    }

    // Preview
    if !output.is_json() {
        if dry_run {
            print_info("Dry run - the following snapshots would be deleted:");
        } else {
            println!("The following snapshots will be deleted:");
        }
        for id in &candidates {
            println!("  {}", id);
        }
        if let Some(ref current) = skipped_current {
            print_warning(&format!(
                "Skipping current snapshot: {} (use 'sys destroy' first)",
                current
            ));
        }
    }

    // Confirmation (unless dry-run or force)
    if !dry_run && !confirm(&format!("Delete {} snapshot(s)?", candidates.len()), force)? {
        if output.is_json() {
            print_json(&DeleteResult {
                deleted: vec![],
                failed: vec![],
                skipped_current,
                dry_run,
            })?;
        } else {
            print_info("Cancelled");
        }
        return Ok(());
    }

    if dry_run {
        if output.is_json() {
            print_json(&DeleteResult {
                deleted: candidates,
                failed: vec![],
                skipped_current,
                dry_run: true,
            })?;
        } else {
            print_info("Dry run - no changes made");
        }
        return Ok(());
    }

    // Acquire lock for actual deletion
    let _lock = StoreLock::acquire(LockMode::Exclusive, "snapshot delete")?;

    // Delete with partial success handling
    let mut deleted = Vec::new();
    let mut failed = Vec::new();

    for id in candidates {
        debug!(snapshot_id = %id, "deleting snapshot");
        match store.delete_snapshot(&id) {
            Ok(()) => {
                info!(snapshot_id = %id, "deleted snapshot");
                deleted.push(id);
            }
            Err(e) => {
                debug!(snapshot_id = %id, error = %e, "failed to delete snapshot");
                failed.push(DeleteFailure {
                    id,
                    error: e.to_string(),
                });
            }
        }
    }

    if output.is_json() {
        print_json(&DeleteResult {
            deleted,
            failed,
            skipped_current,
            dry_run: false,
        })?;
    } else {
        if !deleted.is_empty() {
            print_success(&format!("Deleted {} snapshot(s)", deleted.len()));
        }
        if !failed.is_empty() {
            for f in &failed {
                print_error(&format!("Failed to delete {}: {}", f.id, f.error));
            }
        }
    }

    Ok(())
}

fn cmd_tag(id: &str, name: &str) -> Result<()> {
    let paths = SysluaPaths::new()?;
    let store = SnapshotStore::new(paths.snapshots_dir());

    // Verify snapshot exists
    let _ = store.load_snapshot(id)?;

    let _lock = StoreLock::acquire(LockMode::Exclusive, "snapshot tag")?;
    store.update_snapshot_tag(id, Some(name.to_string()))?;

    info!(snapshot_id = %id, tag = %name, "tagged snapshot");
    print_success(&format!("Tagged snapshot {} as '{}'", id, name));

    Ok(())
}

fn cmd_untag(id: &str) -> Result<()> {
    let paths = SysluaPaths::new()?;
    let store = SnapshotStore::new(paths.snapshots_dir());

    // Verify snapshot exists
    let _ = store.load_snapshot(id)?;

    let _lock = StoreLock::acquire(LockMode::Exclusive, "snapshot untag")?;
    store.update_snapshot_tag(id, None)?;

    info!(snapshot_id = %id, "untagged snapshot");
    print_success(&format!("Removed tag from snapshot {}", id));

    Ok(())
}

/// Format a Unix timestamp as a human-readable string
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    if let Ok(duration) = SystemTime::now().duration_since(datetime) {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{} seconds ago", secs)
        } else if secs < 3600 {
            format!("{} minutes ago", secs / 60)
        } else if secs < 86400 {
            format!("{} hours ago", secs / 3600)
        } else {
            format!("{} days ago", secs / 86400)
        }
    } else {
        // Future timestamp (shouldn't happen)
        format!("timestamp: {}", timestamp)
    }
}
```

### Success Criteria:

#### Automated Verification:

- [x] `cargo build -p syslua-cli` succeeds
- [x] `cargo check -p syslua-cli` passes

#### Manual Verification:

- [x] N/A (tested after Phase 4 integration)

---

## Phase 4: CLI Integration

### Overview

Wire up the snapshot command in main.rs and export from mod.rs.

### Changes Required:

#### 1. Export Snapshot Module

**File**: `crates/cli/src/cmd/mod.rs`

Add module declaration and export:

```rust
mod snapshot;

pub use snapshot::cmd_snapshot;
```

#### 2. Add Snapshot Command to Commands Enum

**File**: `crates/cli/src/main.rs`

Add to `Commands` enum (around line 77-161):

```rust
/// Manage snapshots
Snapshot {
    #[command(subcommand)]
    command: cmd::snapshot::SnapshotCommand,
},
```

Add import at top of file:

```rust
use cmd::snapshot::SnapshotCommand;
```

#### 3. Add Dispatch Handler

**File**: `crates/cli/src/main.rs`

Add to match statement (around line 210-232):

```rust
Commands::Snapshot { command } => cmd::cmd_snapshot(command),
```

### Success Criteria:

#### Automated Verification:

- [x] `cargo build -p syslua-cli` succeeds
- [x] `cargo test -p syslua-cli` passes
- [x] `cargo clippy --all-targets --all-features` passes
- [x] `cargo fmt --check` passes

#### Manual Verification:

- [x] `sys snapshot --help` displays subcommand help
- [x] `sys snapshot list --help` displays list help
- [x] `sys snapshot list` works (shows "No snapshots found" if empty)

---

## Phase 5: Testing

### Overview

Add integration tests for all snapshot operations.

### Changes Required:

#### 1. Create Integration Test Module

**File**: `crates/cli/tests/integration/snapshot_tests.rs` (new file)

```rust
//! Integration tests for the snapshot command.

use super::common::{run_sys, setup_test_env, TempEnv};

/// Test `sys snapshot list` with no snapshots
#[test]
fn test_snapshot_list_empty() {
    let env = setup_test_env("snapshot_list_empty");

    let output = run_sys(&env, &["snapshot", "list"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("No snapshots")
            || String::from_utf8_lossy(&output.stderr).contains("No snapshots")
    );
}

/// Test `sys snapshot list` after an apply
#[test]
fn test_snapshot_list_after_apply() {
    let env = setup_test_env("snapshot_list_apply");

    // Create a minimal config and apply
    std::fs::write(
        env.config_path(),
        r#"
        return {}
        "#,
    )
    .unwrap();

    let apply_output = run_sys(&env, &["apply", "--force"]);
    assert!(apply_output.status.success());

    let output = run_sys(&env, &["snapshot", "list"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show at least one snapshot marked as current
    assert!(stdout.contains("(current)") || stdout.contains("snapshot"));
}

/// Test `sys snapshot list --verbose`
#[test]
fn test_snapshot_list_verbose() {
    let env = setup_test_env("snapshot_list_verbose");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    let output = run_sys(&env, &["snapshot", "list", "--verbose"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verbose should include build/bind counts
    assert!(stdout.contains("builds:") || stdout.contains("binds:"));
}

/// Test `sys snapshot list -o json`
#[test]
fn test_snapshot_list_json() {
    let env = setup_test_env("snapshot_list_json");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    let output = run_sys(&env, &["snapshot", "list", "-o", "json"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should be valid JSON with snapshots array
    assert!(stdout.contains("\"snapshots\""));
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed["snapshots"].is_array());
}

/// Test `sys snapshot show <id>`
#[test]
fn test_snapshot_show() {
    let env = setup_test_env("snapshot_show");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    // Get snapshot ID from list
    let list_output = run_sys(&env, &["snapshot", "list", "-o", "json"]);
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("valid JSON");
    let snapshot_id = list_json["snapshots"][0]["id"]
        .as_str()
        .expect("snapshot ID");

    let output = run_sys(&env, &["snapshot", "show", snapshot_id]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Snapshot:"));
    assert!(stdout.contains(snapshot_id));
}

/// Test `sys snapshot show <id> -o json`
#[test]
fn test_snapshot_show_json() {
    let env = setup_test_env("snapshot_show_json");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    let list_output = run_sys(&env, &["snapshot", "list", "-o", "json"]);
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("valid JSON");
    let snapshot_id = list_json["snapshots"][0]["id"]
        .as_str()
        .expect("snapshot ID");

    let output = run_sys(&env, &["snapshot", "show", snapshot_id, "-o", "json"]);
    assert!(output.status.success());

    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(parsed["id"].as_str(), Some(snapshot_id));
    assert!(parsed["builds"].is_array());
    assert!(parsed["binds"].is_array());
}

/// Test `sys snapshot delete` cannot delete current snapshot
#[test]
fn test_snapshot_delete_current_fails() {
    let env = setup_test_env("snapshot_delete_current");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    let list_output = run_sys(&env, &["snapshot", "list", "-o", "json"]);
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("valid JSON");
    let snapshot_id = list_json["current"].as_str().expect("current ID");

    // Try to delete current snapshot
    let output = run_sys(&env, &["snapshot", "delete", snapshot_id, "--force"]);
    // Should succeed but skip the current snapshot with warning
    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("current") || stderr.contains("destroy"));

    // Snapshot should still exist
    let verify = run_sys(&env, &["snapshot", "show", snapshot_id]);
    assert!(verify.status.success());
}

/// Test `sys snapshot delete --dry-run`
#[test]
fn test_snapshot_delete_dry_run() {
    let env = setup_test_env("snapshot_delete_dry");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    // Create a second snapshot by modifying and reapplying
    std::fs::write(env.config_path(), "return { test = true }").unwrap();
    run_sys(&env, &["apply", "--force"]);

    let list_output = run_sys(&env, &["snapshot", "list", "-o", "json"]);
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("valid JSON");

    // Find non-current snapshot
    let snapshots = list_json["snapshots"].as_array().unwrap();
    let current_id = list_json["current"].as_str();
    let non_current = snapshots
        .iter()
        .find(|s| s["id"].as_str() != current_id)
        .expect("non-current snapshot");
    let snapshot_id = non_current["id"].as_str().unwrap();

    // Dry-run delete
    let output = run_sys(
        &env,
        &["snapshot", "delete", snapshot_id, "--dry-run", "--force"],
    );
    assert!(output.status.success());

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("Dry run") || combined.contains("dry_run"));

    // Snapshot should still exist
    let verify = run_sys(&env, &["snapshot", "show", snapshot_id]);
    assert!(verify.status.success());
}

/// Test `sys snapshot tag` and `sys snapshot untag`
#[test]
fn test_snapshot_tag_untag() {
    let env = setup_test_env("snapshot_tag");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    let list_output = run_sys(&env, &["snapshot", "list", "-o", "json"]);
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_output.stdout).expect("valid JSON");
    let snapshot_id = list_json["snapshots"][0]["id"]
        .as_str()
        .expect("snapshot ID");

    // Tag the snapshot
    let tag_output = run_sys(&env, &["snapshot", "tag", snapshot_id, "my-tag"]);
    assert!(tag_output.status.success());

    // Verify tag appears in list
    let list2 = run_sys(&env, &["snapshot", "list"]);
    let stdout = String::from_utf8_lossy(&list2.stdout);
    assert!(stdout.contains("[my-tag]"));

    // Untag
    let untag_output = run_sys(&env, &["snapshot", "untag", snapshot_id]);
    assert!(untag_output.status.success());

    // Verify tag is gone
    let list3 = run_sys(&env, &["snapshot", "list"]);
    let stdout3 = String::from_utf8_lossy(&list3.stdout);
    assert!(!stdout3.contains("[my-tag]"));
}

/// Test `sys snapshot delete --older-than`
#[test]
fn test_snapshot_delete_older_than() {
    let env = setup_test_env("snapshot_delete_older");

    std::fs::write(env.config_path(), "return {}").unwrap();
    run_sys(&env, &["apply", "--force"]);

    // With only one recent snapshot, --older-than 1s shouldn't match anything
    // (snapshot was just created)
    let output = run_sys(
        &env,
        &["snapshot", "delete", "--older-than", "1s", "--force"],
    );
    assert!(output.status.success());

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Should report nothing to delete or skip current
    assert!(
        combined.contains("No snapshots")
            || combined.contains("current")
            || combined.contains("Cancelled")
    );
}
```

#### 2. Register Test Module

**File**: `crates/cli/tests/integration/mod.rs`

Add module declaration:

```rust
mod snapshot_tests;
```

### Success Criteria:

#### Automated Verification:

- [x] `cargo test -p syslua-cli snapshot` passes
- [x] `cargo test -p syslua-cli` passes (all tests)
- [x] `cargo clippy --all-targets --all-features` passes
- [x] `cargo fmt --check` passes

#### Manual Verification:

- [x] `sys snapshot list` shows all snapshots (newest first)
- [x] `sys snapshot list --verbose` shows extended details
- [x] `sys snapshot list -o json` outputs valid JSON
- [x] `sys snapshot show <id>` displays snapshot details
- [x] `sys snapshot show <id> --verbose` shows builds/binds
- [x] `sys snapshot show <id> -o json` dumps manifest
- [x] `sys snapshot delete <id>` prompts for confirmation
- [x] `sys snapshot delete <id> --force` skips confirmation
- [x] `sys snapshot delete <id> --dry-run` shows preview without deleting
- [x] `sys snapshot delete --older-than 7d` deletes old snapshots with confirmation
- [x] `sys snapshot delete <current-id>` shows warning and skips
- [x] `sys snapshot tag <id> "name"` adds tag
- [x] `sys snapshot untag <id>` removes tag
- [x] Tags appear in `list` and `show` output

---

## Testing Strategy

### Unit Tests:

- `SnapshotMetadata` serialization with/without tag field
- `SnapshotIndex::update_tag()` success and not-found cases
- `SnapshotStore::update_snapshot_tag()` integration

### Integration Tests:

- All subcommands (list, show, delete, tag, untag)
- Output formats (text, JSON)
- Dry-run mode
- Current snapshot protection
- `--older-than` duration filtering
- Partial success on multi-delete

### Manual Testing Steps:

1. Apply a config to create initial snapshot
2. Run `sys snapshot list` - verify snapshot shown with (current)
3. Apply again to create second snapshot
4. Run `sys snapshot list` - verify both shown, one marked current
5. Tag non-current snapshot: `sys snapshot tag <id> "backup"`
6. Verify tag in list output
7. Try `sys snapshot delete <current>` - verify warning
8. Delete non-current with `--dry-run` - verify no deletion
9. Delete non-current with `--force` - verify deleted
10. Run `sys gc` - verify no errors

## Performance Considerations

- `list()` loads entire index into memory - acceptable for typical snapshot counts (<100)
- No pagination implemented (out of scope for v1)
- Tag updates rewrite entire index file (atomic, acceptable)

## Migration Notes

- Existing `index.json` files without the tag field will fail to deserialize - users must delete old snapshots (acceptable pre-1.0)
- No backwards compatibility requirements
- No breaking changes to existing commands

## References

- Original ticket: `thoughts/tickets/feature_snapshot_command.md`
- Related research: `thoughts/research/2025-12-31_snapshot_command.md`
- GC implementation reference: `crates/cli/src/cmd/gc.rs`
- Architecture docs: `docs/architecture/05-snapshots.md`

---

## Deviations from Plan

### Multiple Tags Support

| Aspect | Original Plan | Actual Implementation |
|--------|---------------|----------------------|
| **Field type** | `tag: Option<String>` | `tags: Vec<String>` |
| **Untag command** | `untag <id>` - removes the single tag | `untag <id> [name]` - removes specific tag, or all tags if no name specified |
| **Method names** | `update_tag(id, tag: Option<String>)` | `update_tags(id, tags: Vec<String>)`, `set_snapshot_tags(id, tags)` |

**Reason**: User requested multiple tag support during implementation ("Hold on I thought we were going to support multiple tags").

**Impact**:
- More flexible tagging - snapshots can have multiple labels (e.g., "stable", "backup", "v1.0")
- `tag <id> <name>` adds a tag to existing tags (does not replace)
- `untag <id>` without a name removes all tags
- `untag <id> <name>` removes only the specified tag
- JSON output shows `tags: []` array instead of `tag: null`
- Backwards compatible via `#[serde(default)]` on the `tags` field
