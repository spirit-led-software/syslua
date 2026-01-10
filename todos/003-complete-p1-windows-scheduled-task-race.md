---
status: complete
priority: p1
issue_id: "003"
tags: [code-review, data-integrity, windows, critical]
dependencies: []
---

# Windows Scheduled Task Race Condition

## Problem Statement

The Windows implementation of running `sys apply` as another user uses scheduled tasks with an arbitrary `Start-Sleep -Seconds 2` wait. This creates a race condition where:

1. The task may take longer than 2 seconds to complete
2. The main script continues before `sys apply` finishes
3. If subsequent operations fail, partial state is left behind

## Findings

**Location:** `lua/syslua/user.lua` lines 381-396

```lua
return interpolate(
  [[
$env:SYSLUA_STORE = "{{user_store}}"
$env:SYSLUA_PARENT_STORE = "{{parent_store}}"
$taskName = "SysluaApply_{{username}}"
$action = New-ScheduledTaskAction -Execute "sys" -Argument "apply {{config}}"
$principal = New-ScheduledTaskPrincipal -UserId "{{username}}" -LogonType Interactive
try {
  Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Force | Out-Null
  Start-ScheduledTask -TaskName $taskName
  Start-Sleep -Seconds 2  -- ARBITRARY WAIT
} finally {
  Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
}
]],
```

**Issues:**

1. `Start-Sleep -Seconds 2` is arbitrary - `sys apply` may take much longer
2. No verification that task completed successfully
3. Task is unregistered regardless of completion state
4. Exit code from `sys apply` is lost

## Proposed Solutions

### Option A: Poll for Task Completion (Recommended)

**Pros:** Reliable, waits for actual completion
**Cons:** More complex script
**Effort:** Small
**Risk:** Low

```powershell
Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName $taskName

# Wait for task to complete
$timeout = 300  # 5 minutes
$elapsed = 0
while (($task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) -and
       $task.State -eq 'Running' -and
       $elapsed -lt $timeout) {
  Start-Sleep -Seconds 1
  $elapsed++
}

# Check result
$info = Get-ScheduledTaskInfo -TaskName $taskName
if ($info.LastTaskResult -ne 0) {
  throw "sys apply failed with exit code $($info.LastTaskResult)"
}
```

### Option B: Use Process Impersonation Instead

**Pros:** Direct execution, immediate result
**Cons:** Requires different Windows APIs, more complex
**Effort:** Large
**Risk:** Medium

## Recommended Action

Option A - add proper task completion polling.

## Technical Details

**Affected files:** `lua/syslua/user.lua` lines 370-424

## Acceptance Criteria

- [x] Script waits for scheduled task to actually complete
- [x] Timeout prevents infinite waiting
- [x] Exit code from `sys apply` is captured and propagated
- [x] Task is only unregistered after completion
- [ ] Test on Windows verifies behavior

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during data integrity review of PR #25 |
| 2026-01-10 | Fixed | Replaced arbitrary 2s sleep with completion polling (5min timeout) |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Data Integrity Guardian Agent Report
