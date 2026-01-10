---
status: deferred
priority: p3
issue_id: "001"
tags: [code-review, documentation, hardening]
dependencies: []
---

# Consider Input Validation for Defense in Depth

## Problem Statement

The `lua/syslua/user.lua` module interpolates config values into shell commands without validation. While this is **low risk** given syslua's threat model (users write/trust their own configs, same as Terraform/Nix/Ansible), basic validation could provide defense-in-depth and better error messages for typos.

## Threat Model Context

Users either:

1. Write their own configs (trusted)
2. Use external libraries and accept that risk

This is standard for configuration management tools. The config file IS the trust boundary.

## Optional Improvement

Adding POSIX username validation would catch typos early with clear errors:

```lua
local function validate_username(name)
  if not name:match("^[a-z_][a-z0-9_-]*$") then
    error("Invalid username: must match POSIX naming conventions")
  end
end
```

**Pros:** Better error messages, catches typos, defense in depth
**Cons:** Not strictly necessary given threat model
**Effort:** Small
**Priority:** Nice-to-have

## Acceptance Criteria

- [ ] Consider adding username format validation for better error messages
- [ ] Document that configs are trusted (same as other config mgmt tools)

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during code review of PR #25 |
| 2026-01-10 | Downgraded | P1 -> P3, threat model makes injection low risk |
| 2026-01-10 | Deferred | Nice-to-have documentation task, not blocking for PR merge |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
