# Better Logging Implementation Plan

## Overview

Enhance the logging system to reduce noise, add structured JSON output, enable contextual information, and provide configurable log levels. This addresses the "Better Logging" ticket requirements for improved debugging and monitoring.

## Current State Analysis

The codebase uses `tracing` with `tracing-subscriber` for logging:

- **~185 log statements** in the library crate across 17 files
- **Overuse of INFO level** for internal state changes (e.g., `bind/state.rs` uses INFO for routine persistence operations)
- **Binary log level toggle** via `--debug` flag (INFO vs DEBUG only)
- **No JSON output support** for log messages
- **Target/module paths disabled** (`with_target(false)`)
- **Timestamps only in debug mode**

### Key Files

| File | Purpose |
|------|---------|
| `crates/cli/src/main.rs:11-17,19-32,114-128` | CLI flags and tracing initialization |
| `Cargo.toml:25-27` | Workspace tracing dependencies |
| `crates/lib/src/execute/apply.rs` | Heaviest logging (~55 statements) |
| `crates/lib/src/bind/state.rs` | Most verbose at INFO level |

## Desired End State

After this plan is complete:

1. **Configurable log levels** via `--log-level trace|debug|info|warn|error` flag
2. **JSON log format** via `--log-format text|json` flag  
3. **Contextual information** (module paths) enabled in log output
4. **Reduced log noise** - internal operations demoted to DEBUG, only user-facing events at INFO
5. **TRACE level** available for deep debugging of high-volume internals
6. **Clear documentation** of logging guidelines

### Verification

```bash
# Test log level flag
sys apply --log-level debug ./init.lua  # Shows DEBUG messages
sys apply --log-level warn ./init.lua   # Only WARN and ERROR

# Test JSON format
sys apply --log-format json ./init.lua  # JSON structured output

# Test combined
sys apply --log-level trace --log-format json ./init.lua
```

## What We're NOT Doing

- Adding `#[instrument]` span attributes (deferred to future ticket)
- Adding a separate `--timestamps` flag (timestamps remain debug-mode only)
- Adding `RUST_LOG` environment variable support (using CLI flag instead)
- Changing CLI output (`output.rs`) - that's the separate "better-outputs" concern

## Implementation Approach

The work is organized into 5 phases:
1. Add CLI configuration infrastructure
2. Update tracing subscriber to use new config
3. Audit and fix log levels across the codebase
4. Add TRACE level for high-volume internals
5. Document logging guidelines

Phases 1-2 establish infrastructure. Phase 3 is the bulk of changes. Phases 4-5 are enhancements.

---

## Phase 1: Add CLI Configuration Options

### Overview

Add `--log-level` and `--log-format` global CLI flags following the existing `ColorChoice` pattern.

### Changes Required

#### 1. Update Dependencies

**File**: `Cargo.toml` (workspace root)

Add `json` feature to tracing-subscriber:

```toml
# Line 26 - update existing entry
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

#### 2. Add LogLevel Enum

**File**: `crates/cli/src/main.rs`

After `ColorChoice` enum (line 17), add:

```rust
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}
```

#### 3. Add LogFormat Enum

**File**: `crates/cli/src/main.rs`

After `LogLevel` enum, add:

```rust
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}
```

#### 4. Update CLI Struct

**File**: `crates/cli/src/main.rs`

Update the `Cli` struct (around line 19-32) to add new flags and update `--debug`:

```rust
#[derive(Parser)]
#[command(name = "syslua", author, version, about, long_about = None)]
struct Cli {
    /// Enable debug logging (shorthand for --log-level debug)
    #[arg(short = 'd', long, global = true)]
    debug: bool,

    /// Set log verbosity level
    #[arg(long, value_enum, default_value = "info", global = true)]
    log_level: LogLevel,

    /// Set log output format
    #[arg(long, value_enum, default_value = "text", global = true)]
    log_format: LogFormat,

    /// Control colored output
    #[arg(long, value_enum, default_value = "auto", global = true)]
    color: ColorChoice,

    #[command(subcommand)]
    command: Commands,
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo build -p syslua-cli` compiles without errors
- [ ] `cargo test -p syslua-cli` passes
- [ ] `sys --help` shows new `--log-level` and `--log-format` flags

#### Manual Verification
- [ ] `sys --log-level trace apply ./init.lua` is accepted (doesn't error on flag)
- [ ] `sys --log-format json apply ./init.lua` is accepted

---

## Phase 2: Update Tracing Subscriber Configuration

### Overview

Modify the tracing subscriber initialization to use the new CLI flags for level and format selection.

### Changes Required

#### 1. Update Imports

**File**: `crates/cli/src/main.rs`

Update imports (around line 8-9):

```rust
use tracing::Level;
use tracing_subscriber::{fmt::format::FmtSpan, FmtSubscriber};
```

#### 2. Replace Subscriber Configuration

**File**: `crates/cli/src/main.rs`

Replace the existing subscriber configuration block (lines 120-128) with:

```rust
// Determine effective log level (--debug overrides --log-level)
let level: Level = if cli.debug {
    Level::DEBUG
} else {
    cli.log_level.into()
};

// Configure subscriber based on format choice
match cli.log_format {
    LogFormat::Json => {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(level)
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .json()
            .flatten_event(true);
        
        if cli.debug {
            subscriber.init();
        } else {
            subscriber.without_time().init();
        }
    }
    LogFormat::Text => {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(level)
            .with_target(true);
        
        if cli.debug {
            subscriber.init();
        } else {
            subscriber.without_time().init();
        }
    }
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo build -p syslua-cli` compiles without errors
- [ ] `cargo test -p syslua-cli` passes
- [ ] `cargo clippy -p syslua-cli` passes

#### Manual Verification
- [ ] `sys --log-level debug apply ./init.lua` shows DEBUG level messages
- [ ] `sys --log-level warn apply ./init.lua` only shows WARN/ERROR messages
- [ ] `sys --log-format json apply ./init.lua` outputs JSON structured logs
- [ ] `sys -d apply ./init.lua` still works (shows timestamps + DEBUG level)
- [ ] Log output now includes module paths (e.g., `syslua_lib::execute::apply`)

---

## Phase 3: Audit and Fix Log Levels

### Overview

Review all ~185 log statements and adjust levels appropriately:
- Demote internal operations from INFO to DEBUG
- Promote error conditions from INFO to WARN  
- Keep user-visible operations at INFO

### Changes Required

This phase is organized by file for efficient batch editing.

#### 1. `crates/lib/src/bind/state.rs` (14 statements)

**Demote from `info!` to `debug!`** (10 statements):
- Line 85-90: `"saving bind state"` 
- Line 101: `"bind state saved successfully"`
- Lines 108-112: `"loading bind state"`
- Line 124: `"bind state file found"`
- Line 128: `"bind state file not found"`
- Lines 139-143: `"bind state loaded successfully"`
- Lines 150-154: `"removing bind state directory"`
- Line 158: `"bind state directory removed successfully"`
- Line 162: `"bind state directory already gone"`

**Promote from `info!` to `warn!`** (2 statements):
- Line 132: `"failed to read bind state file"` → `warn!`
- Line 166: `"failed to remove bind state directory"` → `warn!`

**Already correct at `debug!`** (3 statements - no change):
- Line 91, Lines 115-119, Line 138

#### 2. `crates/lib/src/execute/apply.rs` (31 info! statements)

**Demote from `info!` to `debug!`** (16 statements):
- Line 199: `"loaded current state"`
- Line 202: `"evaluating config"`
- Line 420: `"checking unchanged binds for drift"`
- Line 516: `"bind repaired"` (loop iteration)
- Lines 585-590: `"loaded current snapshot"`
- Line 707: `"destroying removed binds"`
- Line 774: `"destroying bind"` (loop iteration)
- Line 786: `"bind destroyed successfully"` (loop iteration)
- Line 789: `"destroy phase complete"`
- Line 833: `"updating modified binds"`
- Line 895: `"updating bind"` (loop iteration)
- Line 921: `"update phase complete"`
- Line 1070: `"bind restored"` (loop iteration)

**Keep at `info!`** (15 statements - no change):
- Line 184: `"starting apply"`
- Lines 205-209: `"config evaluated"` (has counts)
- Lines 215-223: `"diff computed"` (has change summary)
- Line 227: `"no changes to apply"`
- Line 250: `"repaired drifted binds"`
- Line 265: `"dry run - not applying changes"`
- Line 335: `"restored previous snapshot"`
- Line 384: `"snapshot saved"`
- Lines 464-467: `"drift check complete"` (has drift count)
- Line 486: `"repairing drifted binds"`
- Line 541: `"repair complete"`
- Line 562: `"starting destroy"`
- Line 573: `"no current snapshot, nothing to destroy"`
- Line 594: `"no binds to destroy"`
- Line 604: `"dry run - not destroying"`
- Line 649: `"destroy complete"`
- Line 1000: `"restoring destroyed binds"`
- Line 1088: `"restore complete"`

#### 3. `crates/lib/src/execute/mod.rs` (12 info! statements)

**Demote from `info!` to `debug!`** (8 statements):
- Line 58: `"starting build execution"`
- Line 66: `"computed execution waves"`
- Line 114: `"build succeeded"` (loop iteration)
- Lines 163-167: `"starting manifest execution"`
- Line 175: `"computed execution waves"`
- Line 251: `"build succeeded"` (loop iteration)
- Line 283: `"bind succeeded"` (loop iteration)
- Line 509: `"destroying bind"` (loop iteration)

**Keep at `info!`** (4 statements - no change):
- Lines 127-132: `"build execution complete"` (summary with counts)
- Lines 301-308: `"manifest execution complete"` (summary with counts)
- Line 496: `"rolling back applied binds"`
- Line 517: `"rollback complete"`

#### 4. `crates/lib/src/build/execute.rs` (4 info! statements)

**Demote from `info!` to `debug!`** (all 4):
- Lines 140-144: `"realizing build"`
- Lines 211-215: `"build complete"`
- Lines 250-254: `"realizing build (with unified resolver)"`
- Lines 335-339: `"build complete"`

#### 5. `crates/lib/src/bind/execute.rs` (6 info! statements)

**Demote from `info!` to `debug!`** (all 6):
- Line 38: `"applying bind"`
- Line 55: `"bind applied"`
- Line 90: `"destroying bind"`
- Line 108: `"bind destroyed"`
- Line 136: `"updating bind"`
- Line 163: `"bind updated"`

### Success Criteria

#### Automated Verification
- [ ] `cargo build -p syslua-lib` compiles without errors
- [ ] `cargo test` passes (all tests)
- [ ] `cargo clippy --all-targets` passes

#### Manual Verification
- [ ] `sys apply ./init.lua` produces cleaner output (fewer log lines at INFO)
- [ ] `sys --log-level debug apply ./init.lua` shows all the demoted messages
- [ ] Error conditions now show at WARN level

---

## Phase 4: Add TRACE Level for Internals

### Overview

Add `trace!` logging for high-volume internal operations to enable deep debugging without polluting DEBUG output.

### Changes Required

#### 1. `crates/lib/src/placeholder.rs`

Add `trace` to imports and add trace logging for placeholder parsing:

```rust
// Add to imports
use tracing::trace;

// After parsing each placeholder (around line 179):
trace!(placeholder = %placeholder_content, pos, "parsed placeholder");

// In substitute_segments (around line 266-275):
trace!(segment_idx = idx, resolved = %value, "resolved placeholder segment");
```

#### 2. `crates/lib/src/execute/dag.rs`

Add trace logging for DAG construction and traversal:

```rust
// Add to imports
use tracing::trace;

// After adding build node (line 68):
trace!(hash = %hash.0, "added build to DAG");

// After adding bind node (line 74):
trace!(hash = %hash.0, "added bind to DAG");

// After adding dependency edge (line 87):
trace!(from = %dep_hash.0, to = %hash.0, "added dependency edge");

// In topo_visit (line 356):
trace!(path = %path, "visiting DAG node");

// In build_waves wave assignment (around line 389):
trace!(hash = %hash.0, wave = level, "assigned to wave");
```

#### 3. `crates/lib/src/execute/mod.rs`

Add trace to existing imports and add trace logging for wave execution:

```rust
// Update imports to include trace
use tracing::{debug, error, info, trace, warn};

// Before spawning build task (line 360):
trace!(hash = %hash.0, wave = wave_idx, "spawning build task");

// Before spawning bind task (line 407):
trace!(hash = %hash.0, wave = wave_idx, "spawning bind task");
```

#### 4. `crates/lib/src/build/execute.rs`

Add trace for action execution loops:

```rust
// Add trace to imports
use tracing::{debug, info, trace, warn};

// Before execute_action call in loop (line 188):
trace!(action_idx = idx, hash = %hash.0, "executing build action");

// After action result (line 195):
trace!(action_idx = idx, "build action completed");
```

#### 5. `crates/lib/src/bind/execute.rs`

Add trace for bind action execution:

```rust
// Add trace to imports
use tracing::{debug, info, trace};

// Before each action type execution:
// Line 232: trace!(action_idx = idx, hash = %hash.0, "executing check action");
// Line 253: trace!(action_idx = idx, hash = %hash.0, "executing create action");
// Line 277: trace!(action_idx = idx, hash = %hash.0, "executing destroy action");
// Line 298: trace!(action_idx = idx, hash = %hash.0, "executing update action");
```

### Success Criteria

#### Automated Verification
- [ ] `cargo build -p syslua-lib` compiles without errors
- [ ] `cargo test` passes
- [ ] `cargo clippy --all-targets` passes

#### Manual Verification
- [ ] `sys --log-level trace apply ./init.lua` shows TRACE messages
- [ ] `sys --log-level debug apply ./init.lua` does NOT show TRACE messages
- [ ] TRACE output includes placeholder resolution, DAG construction, action execution details

---

## Phase 5: Documentation

### Overview

Update documentation with logging guidelines and new CLI flag usage.

### Changes Required

#### 1. Update AGENTS.md

**File**: `AGENTS.md`

Add a logging section after the existing guidelines (around line 12):

```markdown
- Logging: use existing logging facilities with appropriate levels:
  - `trace!` - High-volume internals (loops, iteration, parsing details)
  - `debug!` - Internal operations, per-item processing, state changes
  - `info!` - User-visible milestones (operation start/complete, summaries with counts)
  - `warn!` - Recoverable errors, unexpected but handled conditions
  - `error!` - Operation failures requiring attention
- Keep log messages actionable; avoid excessive noise at INFO level
- Never log sensitive data (passwords, tokens, env var values) at INFO or above
- Use structured fields: `info!(count = 5, path = %p.display(), "message")`
```

#### 2. Update README.md (if applicable)

Add documentation for new CLI flags in the usage section:

```markdown
### Logging Options

- `--log-level <LEVEL>` - Set log verbosity: `trace`, `debug`, `info` (default), `warn`, `error`
- `--log-format <FORMAT>` - Set log output format: `text` (default), `json`
- `-d, --debug` - Shorthand for `--log-level debug` with timestamps
```

### Success Criteria

#### Automated Verification
- [ ] Documentation files are valid markdown (no syntax errors)

#### Manual Verification
- [ ] AGENTS.md logging guidelines are clear and actionable
- [ ] New developers can understand log level conventions

---

## Testing Strategy

### Unit Tests

No new unit tests required - this is configuration and log level changes.

### Integration Tests

Existing integration tests should continue to pass. Log output changes don't affect test assertions (tests don't typically assert on log content).

### Manual Testing Steps

1. Build and run with each log level to verify filtering works
2. Compare JSON output format against text format
3. Verify module paths appear in log output
4. Test that `--debug` still works as shorthand
5. Verify demoted logs only appear at DEBUG level
6. Verify promoted warnings appear at WARN level

---

## Performance Considerations

- JSON formatting has minimal overhead (only when `--log-format json` specified)
- TRACE level logs are compiled in but filtered at runtime when not enabled
- No performance regression expected for default INFO level

---

## Migration Notes

This is a non-breaking change:
- Default behavior remains INFO level with text format
- `--debug` flag continues to work as before
- Existing scripts/tooling unaffected

---

## References

- Original ticket: `thoughts/tickets/better-logging.md`
- Research document: `thoughts/research/2025-12-29_better-logging.md`
- CLI patterns: `crates/cli/src/main.rs:11-32` (ColorChoice enum pattern)
- Tracing crate docs: https://docs.rs/tracing/latest/tracing/
