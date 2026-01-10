---
status: deferred
priority: p2
issue_id: "005"
tags: [code-review, rust, conventions]
dependencies: []
---

# .expect() Violations in paths.rs

## Problem Statement

The `crates/lib/src/platform/paths.rs` file uses `.expect()` to handle missing environment variables. This violates the project convention from AGENTS.md:
> `.unwrap()` / `.expect()` in library code - Use `?` with proper error types

Panicking in library code prevents consumers from handling errors gracefully.

## Findings

**Location:** `crates/lib/src/platform/paths.rs` lines 13, 36, 43, 50, 66, 82, 95

```rust
// Line 13 (Windows)
let drive = std::env::var("SYSTEMDRIVE").expect("SYSTEMDRIVE not set");

// Line 36
let userprofile = std::env::var("USERPROFILE").expect("USERPROFILE not set");

// Line 43
let home = std::env::var("HOME").expect("HOME not set");

// Line 50
let appdata = std::env::var("APPDATA").expect("APPDATA not set");

// Line 66
let appdata = std::env::var("APPDATA").expect("APPDATA not set");

// Line 82
let local_appdata = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set");

// Line 95
let local_appdata = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set");
```

**Impact:** If these environment variables are missing (unusual but possible), the application panics instead of returning an error that could be handled.

## Proposed Solutions

### Option A: Return Result Types (Recommended)

**Pros:** Proper error handling, follows conventions
**Cons:** Changes function signatures, requires caller updates
**Effort:** Medium
**Risk:** Low

```rust
pub fn home_dir() -> Result<PathBuf, crate::Error> {
  let home = std::env::var("HOME")
    .map_err(|_| crate::Error::MissingEnvVar("HOME"))?;
  Ok(PathBuf::from(home))
}
```

### Option B: Document as Acceptable Exception

**Pros:** No code changes
**Cons:** Inconsistent with project conventions
**Effort:** None
**Risk:** Medium (callers can't handle errors)

These variables are expected to exist on their respective platforms.

## Recommended Action

Option A - align with project conventions.

## Technical Details

**Affected files:** `crates/lib/src/platform/paths.rs`

## Acceptance Criteria

- [ ] All `.expect()` calls replaced with `?` and proper error types
- [ ] Function signatures updated to return `Result<PathBuf, Error>`
- [ ] Callers updated to handle potential errors
- [ ] Error messages are descriptive

## Work Log

| Date | Action | Notes |
|------|--------|-------|
| 2026-01-10 | Identified | Found during pattern recognition review of PR #25 |
| 2026-01-10 | Deferred | Requires updating 10+ callers across codebase. Option B accepted - these env vars are expected to exist on their platforms |

## Resources

- PR #25: <https://github.com/syslua/syslua/pull/25>
- AGENTS.md conventions
- Pattern Recognition Specialist Agent Report
