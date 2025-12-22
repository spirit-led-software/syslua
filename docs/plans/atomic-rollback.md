# Plan: Atomic Apply Rollback

## Goal

Ensure `sys apply` is truly atomic: either all changes succeed or the system remains in its previous state.

## Problem

The current implementation tracks executed nodes but the rollback logic is incomplete. A partial failure could leave the system in an inconsistent state.

## Architecture Reference

- [08-apply-flow.md](../architecture/08-apply-flow.md):203-285 - Atomic apply semantics

## Current State

In `crates/lib/src/execute/apply.rs`:

- `executed_nodes` vector tracks completed nodes
- On failure, destroy is called on bind nodes
- But the full pre-apply state isn't restored

## Approach

1. Create pre-apply snapshot at start of apply
2. Track all executed nodes (builds and binds)
3. On any failure, execute destroy_actions for binds in reverse order
4. Restore to pre-apply snapshot state
5. Report failure with details

## Rollback Flow

```
Apply begins
    |
    +-> Create pre-apply snapshot
    |
    +-> Execute DAG nodes...
    |       |
    |       +-> Node 1: Build (realized) - tracked
    |       +-> Node 2: Bind (applied) - tracked
    |       +-> Node 3: Bind (FAILS)
    |       |
    |       +-> Rollback triggered
    |               |
    |               +-> Execute Node 2 destroy_actions
    |               +-> Restore pre-apply snapshot pointer
    |
    +-> Exit with error (system unchanged)
```

## Files to Modify

| Path                              | Changes                   |
| --------------------------------- | ------------------------- |
| `crates/lib/src/execute/apply.rs` | Improve rollback logic    |
| `crates/lib/src/snapshot/mod.rs`  | Add restore functionality |

## Implementation Details

### Pre-Apply Snapshot

```rust
// Before execution
let pre_apply_snapshot = snapshot_storage.get_current()?;

// ... execute nodes ...

// On failure
if let Some(snapshot) = pre_apply_snapshot {
    snapshot_storage.set_current(snapshot.id)?;
}
```

### Rollback Binds

```rust
// On failure, in reverse order
for node in executed_nodes.iter().rev() {
    if let ManifestNode::Bind(bind_def) = node {
        if let Some(destroy_actions) = &bind_def.destroy_actions {
            for action in destroy_actions {
                execute_action(action, &resolver)?;
            }
        }
    }
}
```

## Success Criteria

1. Failed apply leaves system in pre-apply state
2. All executed binds are destroyed on rollback
3. Pre-apply snapshot is restored as current
4. Clear error message shows which node failed
5. Integration tests verify rollback behavior

## Open Questions

- [ ] What if destroy_actions also fail during rollback?
- [ ] Should we keep failed-apply snapshots for debugging?
- [ ] How to handle builds that were realized (can't un-realize)?
- [ ] Performance implications of always creating pre-apply snapshot?
