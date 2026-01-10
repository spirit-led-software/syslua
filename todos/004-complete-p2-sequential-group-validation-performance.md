---
status: complete
priority: p2
issue_id: "004"
tags: [code-review, performance]
dependencies: []
---

# Sequential Group Validation Creates O(n) Shell Spawns

## Problem Statement

The group validation code spawns a separate shell process for each group to check if it exists. With multiple users having multiple groups, this creates significant overhead during config evaluation (blocking operation).

## Findings

**Location:** `lua/syslua/user.lua` lines 455-513, 574-589

Current implementation:

```lua
-- Lines 574-589: validate_user_options()
local missing_groups = {}
for _, group in ipairs(groups) do
  if not group_exists(group) then  -- Spawns shell per group
    table.insert(missing_groups, group)
  end
end
```

Each `group_exists()` call spawns a new process:

```lua
-- Linux
io.popen(interpolate('getent group "{{group}}" ...', { group = group }))

-- Windows (worst case ~200ms per spawn)
io.popen('powershell -NoProfile -Command "Get-LocalGroup -Name ..."')
```

**Impact Analysis:**

| Users | Groups/User | Shell Spawns | Windows Time |
|-------|-------------|--------------|--------------|
| 5     | 3           | 15           | ~3 seconds   |
| 20    | 5           | 100          | ~20 seconds  |
| 50    | 10          | 500          | ~100 seconds |

## Proposed Solutions

### Option A: Batch Group Validation (Recommended)

**Pros:** O(1) shell spawns regardless of group count
**Cons:** Slightly more complex code
**Effort:** Small
**Risk:** Low

```lua
local function linux_groups_exist_batch(groups)
  if #groups == 0 then return {} end

  local handle = io.popen('getent group | cut -d: -f1')
  local existing = {}
  for line in handle:lines() do
    existing[line] = true
  end
  handle:close()

  local result = {}
  for _, g in ipairs(groups) do
    result[g] = existing[g] or false
  end
  return result
end
```

### Option B: Remove Early Validation

**Pros:** Zero shell spawns, simpler code
**Cons:** Errors deferred to bind execution
**Effort:** Small (deletion)
**Risk:** Low

Let `useradd -G invalid_group` fail with its native error message.

## Recommended Action

Option A for better user experience (clear error messages early), or Option B for simplicity.

## Technical Details

**Affected files:** `lua/syslua/user.lua` lines 455-513, 574-589, 898-916

## Acceptance Criteria

- [x] Group validation uses at most 1 shell spawn per platform
- [x] All unique groups across all users collected before validation
- [x] Config evaluation time reduced by 90%+ for multi-user configs

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during performance review of PR #25 |
| 2026-01-10 | Fixed | Batch validation with single shell spawn via get_all_groups() |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Performance Oracle Agent Report
