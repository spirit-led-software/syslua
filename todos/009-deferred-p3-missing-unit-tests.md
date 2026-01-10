---
status: deferred
priority: p3
issue_id: "009"
tags: [code-review, testing]
dependencies: []
---

# Missing Unit Tests for Lua Helper Functions

## Problem Statement

The `lua/syslua/user.lua` module (643 lines) has only integration test fixtures. There are no unit tests for individual helper functions, making it difficult to verify correctness and catch regressions.

## Findings

**Test coverage analysis:**

| Component | Lines | Has Unit Tests |
|-----------|-------|----------------|
| Command builders | ~200 | No |
| Validation helpers | ~100 | No |
| Group resolution | ~30 | No |
| Path resolution | ~30 | No |

**Existing tests:**

- `crates/lib/tests/fixtures/user/user_basic.lua` - Integration fixture
- `crates/lib/tests/fixtures/user/test_user_config.lua` - Config fixture

These are integration tests that require elevated privileges and create real users.

## Proposed Solutions

### Option A: Add Unit Tests with Mocking (Recommended)

**Pros:** Fast, isolated tests
**Cons:** Need mocking infrastructure for `io.popen`, `sys.*`
**Effort:** Medium
**Risk:** Low

Example:

```lua
-- test/user_test.lua
describe('linux_create_user_cmd', function()
  it('builds correct args for basic user', function()
    local bin, args = linux_create_user_cmd('testuser', {
      homeDir = '/home/testuser',
      description = 'Test User',
    })
    expect(bin).to_equal('/usr/sbin/useradd')
    expect(args).to_contain('-m')
    expect(args).to_contain('-d')
    expect(args).to_contain('/home/testuser')
  end)
end)
```

### Option B: Extract Testable Pure Functions

**Pros:** Functions become easier to test
**Cons:** May require refactoring
**Effort:** Medium
**Risk:** Low

## Recommended Action

Option A - add unit tests for command builder functions and validation helpers.

## Technical Details

**Files to test:** `lua/syslua/user.lua`

**Priority functions to test:**

1. `linux_create_user_cmd` - Verify correct flag construction
2. `darwin_create_user_cmd` - Verify sysadminctl args
3. `windows_create_user_script` - Verify PowerShell script
4. `validate_config_path` - Test file/directory/init.lua resolution
5. `resolve_groups` - Test mergeable handling

## Acceptance Criteria

- [ ] Unit tests for all command builder functions
- [ ] Unit tests for validation helpers
- [ ] Tests verify correct escaping/quoting
- [ ] Tests cover edge cases (empty groups, special characters)
- [ ] CI runs Lua unit tests

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during pattern recognition review of PR #25 |
| 2026-01-10 | Deferred | New feature work requiring Lua test framework setup. Not blocking for PR merge. |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Pattern Recognition Specialist Agent Report
