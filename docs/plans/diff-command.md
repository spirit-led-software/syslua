# Plan: `sys diff` Command

## Goal

Implement the `sys diff` command to compare two snapshots and show what changed.

## Problem

Users have no visibility into what changed between configurations or how snapshots differ. The `StateDiff` logic exists in the library but isn't exposed via CLI.

## Architecture Reference

- [05-snapshots.md](../architecture/05-snapshots.md):319-337 - Comparing snapshots

## Approach

1. Add `diff` subcommand to CLI
2. Load two snapshots (current vs previous, or two specified IDs)
3. Use existing `StateDiff::compute()` to find differences
4. Format output to show added/removed/unchanged builds and binds

## CLI Interface

```bash
sys diff                            # Compare current to previous
sys diff <snapshot_a> <snapshot_b>  # Compare two specific snapshots
sys diff --json                     # Output as JSON
```

## Expected Output Format

```
Build changes:
  + ripgrep-16.0.0-newhash  (new version)
  - ripgrep-15.1.0-oldhash  (removed)
  = neovim-0.10.0-abc123    (unchanged)

Bind changes:
  ~ Symlink ~/.gitconfig    (different build: def456 -> ghi789)
  + Service postgresql      (added)
  - Symlink /old/tool       (removed)
```

## Files to Create

| Path | Purpose |
|------|---------|
| `crates/cli/src/cmd/diff.rs` | CLI command implementation |

## Files to Modify

| Path | Changes |
|------|---------|
| `crates/cli/src/cmd/mod.rs` | Add diff module |
| `crates/cli/src/main.rs` | Add diff subcommand |

## Success Criteria

1. `sys diff` shows differences between current and previous snapshots
2. Output clearly shows added, removed, and unchanged items
3. JSON output mode for scripting
4. Handles case where snapshots don't exist gracefully

## Open Questions

- [ ] Should diff also show the actual actions that would execute?
- [ ] Color output for terminal?
- [ ] Should there be a `--summary` mode for just counts?
