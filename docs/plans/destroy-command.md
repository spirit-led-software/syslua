# Plan: `sys destroy` Command

## Goal

Implement the `sys destroy` command to tear down all binds defined by a configuration, effectively removing everything syslua has applied.

## Problem

The `sys destroy` command is currently a placeholder that only prints a message. Users have no way to cleanly remove all managed state from their system.

## Architecture Reference

- [02-binds.md](../architecture/02-binds.md) - Binds have `destroy_actions` that reverse their effects
- [05-snapshots.md](../architecture/05-snapshots.md) - Destroy should respect snapshot state

## Approach

1. Load the current snapshot from the store
2. For each bind in the manifest, execute its `destroy_actions` in reverse order
3. Remove bind state files from `store/bind/<hash>/`
4. Clear the current snapshot pointer
5. Optionally prompt for confirmation before destruction

## Key Considerations

- Should `destroy` require the original config file, or work from the current snapshot?
- Should builds be deleted from the store, or left for GC?
- How to handle partial failures during destroy?
- Should there be a `--force` flag to skip confirmation?

## Files to Modify

| Path                            | Changes                                 |
| ------------------------------- | --------------------------------------- |
| `crates/cli/src/cmd/destroy.rs` | Full implementation                     |
| `crates/lib/src/execute/mod.rs` | Add `destroy_all()` or similar function |

## Success Criteria

1. `sys destroy init.lua` removes all binds from the current snapshot
2. Destroy actions are executed in reverse order of apply
3. Bind state files are cleaned up
4. Appropriate error handling for failed destroy actions
5. Integration tests verify destroy behavior

## Open Questions

- [ ] Should destroy work without the config file (pure snapshot-based)?
- [ ] What happens if destroy is run when no snapshot exists?
- [ ] Should destroy create a "destroyed" snapshot for audit trail?
