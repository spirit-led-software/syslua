---
status: pending
priority: p1
issue_id: "002"
tags: [code-review, data-integrity, critical]
dependencies: []
---

# Boolean Output Serialization Bug in preserve_home

## Problem Statement

The `preserve_home` field in bind outputs is a boolean, but the Rust bind system serializes outputs as `BTreeMap<String, String>`. When `preserve_home = false` is serialized and passed back to the Lua destroy function, it becomes the string `"false"`, which is **truthy in Lua**. This causes home directories to be preserved when they should be deleted.

## Findings

**Location:** `lua/syslua/user.lua` lines 792-797 (create outputs), lines 819, 849, 879 (destroy usage)

Create function returns:

```lua
return {
  username = inputs.username,
  home_dir = inputs.home_dir,
  preserve_home = inputs.preserve_home,  -- boolean
}
```

Destroy function uses:

```lua
local _, delete_args = linux_delete_user_cmd(outputs.username, outputs.preserve_home)
```

The `linux_delete_user_cmd` expects a boolean, but receives a string.

**Rust serialization:** From `crates/lib/src/outputs/lua.rs`:

```rust
pub fn parse_outputs(table: LuaTable) -> LuaResult<BTreeMap<String, String>> {
  for pair in table.pairs::<String, String>() {  // Only accepts strings!
```

**Impact:** Users with `preserveHomeOnRemove = false` will have their home directories preserved instead of deleted, leaving orphaned data.

## Proposed Solutions

### Option A: Convert to String in Create (Recommended)

**Pros:** Simple, explicit
**Cons:** Requires convention documentation
**Effort:** Small
**Risk:** Low

```lua
return {
  username = inputs.username,
  home_dir = inputs.home_dir,
  preserve_home = tostring(inputs.preserve_home),  -- "true" or "false"
}
```

And in destroy:

```lua
local preserve = outputs.preserve_home == "true"
local _, delete_args = linux_delete_user_cmd(outputs.username, preserve)
```

### Option B: Fix Rust to Handle Booleans

**Pros:** Fixes issue at root
**Cons:** Larger change, affects all binds
**Effort:** Medium
**Risk:** Medium

Modify `parse_outputs` to handle `LuaValue::Boolean`.

## Recommended Action

Option A - fix in Lua code to match existing serialization behavior.

## Technical Details

**Affected files:**

- `lua/syslua/user.lua` - lines 792-797, 819, 849, 879

## Acceptance Criteria

- [ ] `preserve_home` is correctly serialized as string "true" or "false"
- [ ] Destroy function correctly parses string back to boolean
- [ ] Home directories are deleted when `preserveHomeOnRemove = false`
- [ ] Home directories are preserved when `preserveHomeOnRemove = true`
- [ ] Test case added for both scenarios

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during data integrity review of PR #25 |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Data Integrity Guardian Agent Report
