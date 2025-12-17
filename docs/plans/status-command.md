# Plan: `sys status` Command

## Goal

Implement the `sys status` command to show the current system state managed by syslua.

## Problem

Users have no quick way to see what syslua is currently managing on their system without re-running the full config evaluation.

## Architecture Reference

- [05-snapshots.md](../architecture/05-snapshots.md) - Current snapshot contains all managed state
- [03-store.md](../architecture/03-store.md) - Store layout

## Approach

1. Add `status` subcommand to CLI
2. Load current snapshot from store
3. Display summary of managed builds and binds
4. Optionally show detailed information

## CLI Interface

```bash
sys status                  # Show summary of current state
sys status --verbose        # Show all builds and binds
sys status --json           # Output as JSON
```

## Expected Output Format

```
Current snapshot: abc123-def456
Created: 2024-12-17 10:30:00

Builds: 5
  ripgrep-15.1.0-abc123
  fd-9.0.0-def456
  neovim-0.10.0-ghi789
  ...

Binds: 8
  /usr/local/bin/rg -> store/obj/ripgrep-15.1.0-abc123/bin/rg
  ~/.gitconfig -> store/obj/file-gitconfig-xyz789/content
  ...

Store usage: 156 MB
```

## Files to Create

| Path | Purpose |
|------|---------|
| `crates/cli/src/cmd/status.rs` | CLI command implementation |

## Files to Modify

| Path | Changes |
|------|---------|
| `crates/cli/src/cmd/mod.rs` | Add status module |
| `crates/cli/src/main.rs` | Add status subcommand |

## Success Criteria

1. `sys status` shows current snapshot summary
2. Verbose mode lists all builds and binds
3. Handles case where no snapshot exists
4. JSON output for scripting

## Open Questions

- [ ] Should status verify that binds are still intact (symlinks exist)?
- [ ] Should it show store disk usage?
- [ ] Should it integrate with `sys info` or be separate?
