---
date: 2025-12-29T15:27:08-05:00
git_commit: 9c61eebeca23e9e6bf54ef3e97bd0d8bde92bd0e
branch: feat/sys-status-and-diff
repository: syslua
topic: "Better Logging System"
tags: [research, logging, tracing, structured-logging, observability]
last_updated: 2025-12-29
---

## Ticket Synopsis

Enhance the logging system to provide more detailed, structured, and user-friendly log messages. The requirements are:

1. Remove low-value log messages that do not contribute to understanding
2. Introduce log levels (DEBUG, INFO, WARNING, ERROR, CRITICAL) where appropriate
3. Implement structured logging (JSON format) for easier parsing and analysis
4. Include contextual information (function names, line numbers) in log messages
5. Add timestamps to all log entries
6. Provide configuration options for verbosity and output formats
7. Ensure sensitive information is not logged

## Summary

The codebase uses the `tracing` crate with `tracing-subscriber` for logging. The current implementation is functional but has several areas for improvement:

- **~185 log statements** exist in the library crate, concentrated in `execute/apply.rs` (~55) and `execute/mod.rs` (~25)
- **Overuse of INFO level** for internal state changes that should be DEBUG
- **No JSON output support** for log messages (separate from CLI `--json` flag)
- **Timestamps only in debug mode** - controlled via `--debug` flag
- **No span/instrumentation** - No `#[instrument]` attributes or span usage
- **Target disabled** - Module paths not shown in log output

The "better-outputs" ticket has overlapping concerns but is fundamentally different:
- **Better Outputs** = User-facing CLI UX (what the user sees)
- **Better Logging** = Developer/debug internal tracing (what's logged for debugging)

## Detailed Findings

### Current Logging Infrastructure

#### Dependencies (Cargo.toml, workspace level)
```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-test = "0.2"  # for tests
```

#### Initialization (crates/cli/src/main.rs:120-127)
```rust
let level = if cli.debug { Level::DEBUG } else { Level::INFO };
let subscriber = FmtSubscriber::builder()
  .with_max_level(level)
  .with_target(false);  // Module path DISABLED

if cli.debug {
  subscriber.init();          // With timestamps
} else {
  subscriber.without_time().init();  // No timestamps
}
```

Key observations:
- Two-level toggle only (DEBUG vs INFO)
- Target/module paths disabled
- Timestamps tied to debug flag

### Log Statement Distribution

| File | Count | Levels Used | Notes |
|------|-------|-------------|-------|
| `execute/apply.rs` | ~55 | debug, info, warn, error | Heaviest logging |
| `execute/mod.rs` | ~25 | debug, info, warn, error | Wave execution |
| `build/execute.rs` | ~18 | debug, info, warn | Build operations |
| `inputs/resolve.rs` | ~17 | debug, info, trace, warn | Only file using trace! |
| `bind/state.rs` | ~14 | debug, info | **Overly verbose at INFO** |
| `bind/execute.rs` | ~14 | debug, info | Bind operations |
| `action/actions/exec.rs` | 6 | debug, info | Command execution |
| `inputs/fetch.rs` | 5 | debug, info | Git operations |
| `inputs/graph.rs` | 4 | debug, trace, warn | Dependency graph |
| `platform/immutable.rs` | 4 | debug, warn | Store paths |
| Other files | ~20 | Various | Sparse usage |

**Total: ~185 log statements in library crate, 5 in CLI crate**

### Low-Value Log Messages (Candidates for Demotion)

#### bind/state.rs - Most Verbose
```rust
// Lines 85-166: These should ALL be DEBUG, not INFO
info!(hash = %hash.0, path = %path.display(), "saving bind state");
info!(hash = %hash.0, "bind state saved successfully");
info!(hash = %hash.0, path = %path.display(), "loading bind state");
info!(hash = %hash.0, content_len = content.len(), "bind state file found");
info!(hash = %hash.0, path = %path.display(), "bind state file not found");
info!(hash = %hash.0, "removing bind state directory");
```

**Recommendation**: Demote all to DEBUG - these are internal persistence operations.

#### execute/apply.rs - Duplicate Entry/Exit Patterns
```rust
// Lines 199-208: Evaluating config
info!("evaluating config");
info!(builds = ..., binds = ..., "config evaluated");
```

**Recommendation**: Keep only the completion log with context, demote start to DEBUG.

#### execute/mod.rs - Wave Counting
```rust
// Line 58: Implementation detail
info!(wave_count = waves.len(), "computed execution waves");
```

**Recommendation**: Demote to DEBUG.

### Log Level Usage Assessment

| Level | Current Usage | Appropriate? |
|-------|---------------|--------------|
| `trace!` | Only in `inputs/resolve.rs`, `inputs/graph.rs` | Underused |
| `debug!` | Used for detailed operations | Appropriate |
| `info!` | Overused for internal state | Needs audit |
| `warn!` | Used for recoverable issues | Appropriate |
| `error!` | Only in `execute/` module | Appropriate |
| CRITICAL/FATAL | Not used | Not needed (Rust uses panic!) |

### Structured Logging Patterns

The codebase already uses structured fields correctly:

```rust
// Good patterns already in use
info!(hash = %hash.0, "build succeeded");
info!(builds = 5, binds = 3, "config evaluated");
debug!(path = %path.display(), "resolved path");
error!(build = %hash.0, error = %e, "build failed");

// % = Display trait, ? = Debug trait
debug!(outputs = ?state.outputs, "bind state outputs");
```

**Missing**: JSON subscriber configuration for log aggregation.

### JSON Output Support (Implementation)

Required changes to `main.rs`:

```rust
use tracing_subscriber::fmt::format::FmtSpan;

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum LogFormat {
  #[default]
  Text,
  Json,
}

// In Cli struct:
#[arg(long, value_enum, default_value = "text", global = true)]
log_format: LogFormat,

// In main():
match cli.log_format {
  LogFormat::Json => {
    subscriber
      .json()
      .with_file(true)
      .with_line_number(true)
      .with_current_span(true)
      .init();
  }
  LogFormat::Text => { /* existing logic */ }
}
```

**Dependency update needed**:
```toml
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
```

### Contextual Information

Currently disabled (`with_target(false)`). To enable:

```rust
let subscriber = FmtSubscriber::builder()
  .with_max_level(level)
  .with_target(true)          // Enable module::function path
  .with_file(true)            // Enable source file
  .with_line_number(true);    // Enable line numbers
```

For function names, add `#[instrument]` attributes to key functions:

```rust
#[tracing::instrument(skip(manifest), fields(config = %config_path.display()))]
pub async fn apply(config_path: &Path, ...) -> Result<ApplyResult> {
```

### Timestamps

Current behavior:
- Debug mode (`-d`): timestamps shown
- Normal mode: timestamps hidden (`without_time()`)

The ticket requests "timestamps on all log entries" which conflicts with the better-outputs decision (debug-only). Options:

1. **Always show timestamps** - Change default behavior
2. **Separate flag** - `--timestamps` independent of `--debug`
3. **Keep current** - Timestamps in debug mode only

**Recommendation**: Add separate `--timestamps` flag for explicit control.

### Security Analysis

**No sensitive data logging found.** Grep for patterns like `password|secret|token|credential|api.?key` found only:

1. **Windows API token** (`platform/mod.rs:73-88`) - Process token for privilege checking, not a secret
2. **Test fixture** (`action/actions/fetch_url.rs:148`) - URL with `?token=abc` in test only

**Potential risk points**:
- `ExecOpts.env` could contain secrets but is only logged at DEBUG level via derive(Debug)
- User-provided exec commands could have sensitive arguments

**Recommendation**: Add documentation in AGENTS.md about not logging env vars at INFO or above.

### Configuration Options

| Option | Current | Recommended |
|--------|---------|-------------|
| Log level | `-d/--debug` (binary) | Add `-v/--verbose` for TRACE |
| Log format | Text only | Add `--log-format text\|json` |
| Timestamps | Debug-only | Add `--timestamps` flag |
| Color | `--color auto\|always\|never` | Already exists |
| Filter | None | Support `RUST_LOG` env var via EnvFilter |

## Code References

### Core Logging Infrastructure
- `Cargo.toml:25-27` - Workspace tracing dependencies
- `crates/cli/src/main.rs:8-9,120-127` - Tracing initialization
- `crates/cli/src/main.rs:21-23` - CLI debug flag definition

### Heaviest Logging Files
- `crates/lib/src/execute/apply.rs` - ~55 log statements
- `crates/lib/src/execute/mod.rs` - ~25 log statements
- `crates/lib/src/bind/state.rs` - ~14 statements (overly verbose)

### CLI Output (Separate Concern)
- `crates/cli/src/output.rs` - CLI output helpers (NOT logging)
- `crates/cli/src/cmd/*.rs` - Command-specific output

## Architecture Insights

### Tracing vs CLI Output Separation

The codebase correctly separates two concerns:

1. **Tracing** (`tracing` crate)
   - Internal debugging/observability
   - Controlled by `-d/--debug`
   - Goes to stderr by default
   - For developers/operators

2. **CLI Output** (`output.rs` helpers)
   - User-facing results
   - Controlled by `--json`, `--verbose` per command
   - Goes to stdout
   - For end users

This separation should be maintained. The "better logging" ticket affects only the tracing side.

### Logging Patterns in Use

Good patterns already established:
- Structured fields with `%` (Display) and `?` (Debug)
- Entry/exit logging for operations
- Error context with `error = %e`
- Hash identifiers as primary context

Areas needing improvement:
- No span-based tracing (`#[instrument]`)
- Inconsistent log levels
- No trace-level for high-volume internals

## Historical Context (from thoughts/)

### Related Documents

| Document | Relevance |
|----------|-----------|
| `thoughts/tickets/better-outputs.md` | Overlapping but separate concern - CLI UX |
| `thoughts/plans/better-cli-outputs.md` | Implementation plan for CLI outputs |
| `thoughts/research/2025-12-29_better-cli-outputs.md` | Research on CLI improvements |

### Key Decisions from better-cli-outputs

1. **`--verbose` renamed to `--debug`** - Global flag controls tracing level
2. **Timestamps in debug mode only** - Conflicts with this ticket's requirement
3. **`owo-colors` for CLI coloring** - Not relevant to logging
4. **Shared `output.rs` module exists** - For CLI output, not logging

### Overlap Resolution

| Feature | Better Outputs Ticket | Better Logging Ticket | Resolution |
|---------|----------------------|----------------------|------------|
| Timestamps | Debug-only | All logs | Separate `--timestamps` flag |
| JSON format | CLI `--json` | Log format | Separate `--log-format` flag |
| Log levels | Exists (`--debug`) | Audit existing | Review ~185 statements |
| Context | N/A | Function/line info | Enable in subscriber |

## Recommendations

### Priority 1: Reduce Log Noise (High Impact, Low Effort)

1. **Demote bind/state.rs logs** to DEBUG
   - All 14 statements are internal persistence operations
   - User doesn't need to see bind state file operations

2. **Demote "starting X" logs** to DEBUG, keep "X complete" at INFO
   - Pattern: `info!("evaluating config")` → `debug!("evaluating config")`

3. **Consolidate wave execution logs**
   - Current: Multiple logs per wave
   - Proposed: Single summary at completion

### Priority 2: Add JSON Log Support (High Impact, Medium Effort)

1. Add `--log-format text|json` global flag
2. Update tracing-subscriber features to include "json"
3. Configure JSON subscriber with file/line info

### Priority 3: Enable Context Information (Medium Impact, Low Effort)

1. Set `.with_target(true)` in subscriber configuration
2. Optionally add `.with_file(true).with_line_number(true)` for JSON format
3. Consider adding `#[instrument]` to key entry points

### Priority 4: Separate Timestamps Control (Medium Impact, Low Effort)

1. Add `--timestamps` global flag
2. Make independent of `--debug`
3. Default: off (current behavior)

### Priority 5: Environment Filter Support (Low Impact, Low Effort)

1. Support `RUST_LOG` environment variable
2. Use `EnvFilter::try_from_default_env()` with fallback

### Priority 6: Add TRACE Level Usage (Low Impact, Medium Effort)

1. Add `trace!` for placeholder resolution details
2. Add `trace!` for individual action execution
3. Add `trace!` for DAG traversal internals

## Open Questions

1. **Timestamps default**: Should timestamps be on by default, or require `--timestamps`?

2. **Log level granularity**: Is DEBUG/INFO/TRACE sufficient, or do we need per-module filtering?

3. **Span instrumentation**: Worth the effort to add `#[instrument]` attributes systematically?

4. **CRITICAL level**: Rust doesn't have CRITICAL - is `error!` + `panic!` sufficient?

5. **Sequencing**: Should this wait until better-outputs is complete to avoid conflicts?
