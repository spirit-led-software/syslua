---
status: deferred
priority: p2
issue_id: "006"
tags: [code-review, security]
dependencies: []
---

# Plaintext Password Handling

## Problem Statement

The `initialPassword` field stores and transmits passwords in plaintext, creating multiple security concerns:

- Visible in process listings (`ps aux`)
- May be logged in tracing/debug output
- Stored in Lua configuration files
- Passed as command-line arguments (visible to all users on the system)

## Findings

**Location:** `lua/syslua/user.lua` lines 16, 114-117, 203-217, 694-700

Type definition:

```lua
---@field initialPassword? syslua.Option<string> Initial password (plaintext, set on creation only)
```

Linux implementation:

```lua
ctx:exec({
  bin = '/bin/sh',
  args = {
    '-c',
    interpolate(
      'echo "{{username}}:{{password}}" | chpasswd',
      { username = inputs.username, password = inputs.initial_password }
    ),
  },
})
```

Windows implementation:

```lua
'$securePass = ConvertTo-SecureString "{{password}}" -AsPlainText -Force'
```

**Note:** The design spec acknowledges this: "Future: SOPS integration for encrypted secrets."

## Proposed Solutions

### Option A: Use stdin Instead of Command Line (Recommended)

**Pros:** Password not visible in process list
**Cons:** More complex command construction
**Effort:** Medium
**Risk:** Low

Linux example using stdin:

```lua
ctx:exec({
  bin = '/usr/sbin/chpasswd',
  args = {},
  stdin = inputs.username .. ':' .. inputs.initial_password,
})
```

### Option B: Hash Password Before Passing

**Pros:** Plaintext never on command line
**Cons:** Requires password hashing library
**Effort:** Medium
**Risk:** Low

Use `mkpasswd` or similar to pre-hash, then use `usermod -p`.

### Option C: Remove Password Support

**Pros:** Eliminates security concern entirely
**Cons:** Reduces functionality
**Effort:** Small
**Risk:** None

Defer password setting to manual process or SOPS integration.

## Recommended Action

Option A for immediate improvement; Option C with SOPS integration for long-term solution.

## Technical Details

**Affected files:** `lua/syslua/user.lua` lines 16, 114-117, 203-217, 694-700

## Acceptance Criteria

- [ ] Passwords not visible in `ps aux` output
- [ ] Passwords not logged by tracing
- [ ] Documentation warns about plaintext storage in config files
- [ ] Consider SOPS integration roadmap

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during security review of PR #25 |
| 2026-01-10 | Deferred | Requires stdin support in action framework (Rust changes). Current exec() doesn't support stdin piping |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- Security Sentinel Agent Report
