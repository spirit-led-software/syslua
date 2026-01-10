---
status: complete
priority: p3
issue_id: "008"
tags: [code-review, code-quality, lua]
dependencies: []
---

# @diagnostic disable-next-line Type Bypasses

## Problem Statement

The code uses `@diagnostic disable-next-line: missing-fields` annotations to suppress type errors when constructing partial options objects. This bypasses type safety.

## Findings

**Location:** `lua/syslua/user.lua` lines 661, 670, 705, 714, 739, 747

```lua
---@diagnostic disable-next-line: missing-fields
local _, create_args = linux_create_user_cmd(inputs.username, {
  description = inputs.description,
  homeDir = inputs.home_dir,
  shell = inputs.shell,
  groups = inputs.groups,
})
```

**Issue:** The code constructs partial `Options` objects inline, missing required fields like `config`, then suppresses the type error.

## Proposed Solutions

### Option A: Define Partial Type (Recommended)

**Pros:** Type safety maintained
**Cons:** More type definitions
**Effort:** Small
**Risk:** None

```lua
---@class syslua.user.CreateCommandOptions
---@field description? string
---@field homeDir string
---@field shell? BuildRef
---@field groups? string[]
---@field initialPassword? string

---@param name string
---@param opts syslua.user.CreateCommandOptions  -- Use specific type
---@return string bin, string[] args
local function linux_create_user_cmd(name, opts)
```

### Option B: Pass Individual Parameters

**Pros:** No partial objects needed
**Cons:** Long parameter lists
**Effort:** Medium
**Risk:** Low

## Recommended Action

Option A - define specific types for command builder functions.

## Technical Details

**Affected files:** `lua/syslua/user.lua` lines 661, 670, 705, 714, 739, 747

## Acceptance Criteria

- [x] All `@diagnostic disable-next-line` annotations removed
- [x] Proper types defined for command builder inputs
- [x] Type checker passes without suppressions

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during pattern recognition review of PR #25 |
| 2026-01-10 | Completed | Implemented Option A with specific types: CreateCmdOpts, UpdateCmdOpts, DescriptionOnlyOpts, Defaults |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Pattern Recognition Specialist Agent Report
