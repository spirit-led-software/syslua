---
status: complete
priority: p3
issue_id: "010"
tags: [code-review, code-quality, error-handling]
dependencies: []
---

# Silent Failure in group_exists Functions

## Problem Statement

The `*_group_exists` functions return `false` when `io.popen` fails, making it impossible to distinguish between "group doesn't exist" and "command failed to execute."

## Findings

**Location:** `lua/syslua/user.lua` lines 455-513

```lua
local function linux_group_exists(group)
  local handle = io.popen(interpolate('getent group "{{group}}" ...', { group = group }))
  if not handle then
    return false  -- Silent failure - could be popen failure or group missing
  end
  local result = handle:read('*a'):gsub('%s+', '')
  handle:close()
  return result == 'yes'
end
```

**Impact:** If `io.popen` fails (e.g., permission denied, out of memory), the function returns `false` suggesting the group doesn't exist. The validation then fails with a confusing error message about missing groups.

## Proposed Solutions

### Option A: Return Error Tuple (Recommended)

**Pros:** Callers can distinguish failure modes
**Cons:** Requires caller changes
**Effort:** Small
**Risk:** Low

```lua
---@return boolean exists, string? error
local function linux_group_exists(group)
  local handle = io.popen(...)
  if not handle then
    return false, 'Failed to execute getent'
  end
  local result = handle:read('*a'):gsub('%s+', '')
  handle:close()
  return result == 'yes', nil
end
```

### Option B: Throw on Command Failure

**Pros:** Clear failure
**Cons:** May be too aggressive
**Effort:** Small
**Risk:** Medium

```lua
if not handle then
  error('Failed to check group existence: io.popen failed')
end
```

## Recommended Action

Option A - return error information for proper handling.

## Technical Details

**Affected files:** `lua/syslua/user.lua` lines 455-513

## Acceptance Criteria

- [x] Functions distinguish between "not found" and "command failed"
- [x] Callers handle error returns appropriately
- [x] Clear error messages when commands fail

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during pattern recognition review of PR #25 |
| 2026-01-10 | Completed | Implemented Option B in batch group functions (linux_get_all_groups, darwin_get_all_groups, windows_get_all_groups) - throw error on io.popen failure |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Pattern Recognition Specialist Agent Report
