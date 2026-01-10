---
status: deferred
priority: p2
issue_id: "007"
tags: [code-review, data-integrity, architecture]
dependencies: []
---

# Two-Phase Destroy Without Rollback

## Problem Statement

The destroy sequence is:

1. Run `sys destroy` as user (removes user's managed state)
2. Delete user account

If step 2 fails but step 1 succeeded, the user account still exists but their managed configurations are gone. There's no rollback mechanism to restore the user's state.

## Findings

**Location:** `lua/syslua/user.lua` lines 799-892

```lua
destroy = function(outputs, ctx)
  -- Step 1: Destroy user's syslua config (only if user exists)
  local _, destroy_args = unix_destroy_as_user_cmd(outputs.username, outputs.home_dir)
  ctx:exec({...destroy_cmd...})  -- If this succeeds...

  -- Step 2: Remove user account (only if user exists)
  local _, delete_args = linux_delete_user_cmd(outputs.username, outputs.preserve_home)
  ctx:exec({...delete_cmd...})  -- ...but this fails, state is lost
end
```

**Failure scenarios:**

- User logged in with active processes → `userdel` fails
- File system permissions issue → `userdel` fails
- macOS requires admin password → `sysadminctl` fails

## Proposed Solutions

### Option A: Reverse Destruction Order

**Pros:** If user deletion fails, config is still intact
**Cons:** User must be deleted before config can be destroyed (user doesn't exist)
**Effort:** N/A
**Risk:** N/A
**Note:** This doesn't work - can't run as user after deleting them.

### Option B: Add Warning/Documentation (Recommended)

**Pros:** Users understand the risk
**Cons:** Doesn't fix the fundamental issue
**Effort:** Small
**Risk:** None

Document that destroy may leave partial state if user cannot be deleted.

### Option C: Add Backup Before Destroy

**Pros:** Can restore if second phase fails
**Cons:** Complex, requires backup infrastructure
**Effort:** Large
**Risk:** Medium

Snapshot user's store before destroying, restore if delete fails.

### Option D: Kill User Processes First

**Pros:** Reduces likelihood of failure
**Cons:** Forceful, may interrupt user work
**Effort:** Medium
**Risk:** Medium

Add `pkill -u username` before user deletion.

## Recommended Action

Option B (documentation) plus Option D (kill processes) for robustness.

## Technical Details

**Affected files:** `lua/syslua/user.lua` lines 799-892

## Acceptance Criteria

- [ ] Documentation explains destroy sequence and failure modes
- [ ] Consider adding process termination before user deletion
- [ ] Add retry logic for transient failures

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during data integrity review of PR #25 |
| 2026-01-10 | Deferred | Architectural limitation. Options require either documentation or process termination logic. Defer to post-MVP |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Data Integrity Guardian Agent Report
- Architecture Strategist Agent Report
