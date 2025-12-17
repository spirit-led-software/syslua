# Plan: CLI Integration Tests

## Goal

Add smoke tests for the CLI commands (`apply`, `plan`, `destroy`, `init`, `update`, `info`). Currently the entire `syslua-cli` crate has **zero tests**, which is a critical gap.

## Problem

The CLI is the primary user interface for syslua, yet:

- No smoke tests verify commands run without panicking
- No tests verify correct exit codes
- Regressions in CLI behavior go undetected until users report them

## Approach

Use Rust's built-in test framework with `assert_cmd` and `predicates` crates for CLI testing. Tests will:

1. Build the CLI binary once per test run
2. Execute commands against temporary directories
3. Verify exit codes and basic stdout/stderr content

## Prerequisites

Add test dependencies to `crates/cli/Cargo.toml`:

```toml
[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
serial_test = "3"
```

Note: `serial_test` is required because tests use the `SYSLUA_USER_STORE` environment variable
for store isolation, and parallel tests could interfere with each other.

## Test Structure

Create a single test file:

```
crates/cli/tests/
└── cli_smoke.rs       # All smoke tests in one file
```

## CLI Details

Based on the current implementation:

- **Binary name**: `syslua-cli` (not `sys`)
- **Argument style**: Positional arguments for config paths (not `--config` flags)
  - `apply <file>`, `plan <file>`, `destroy <file>`
  - `init <path>`
  - `update [CONFIG]` (optional, with `--dry-run` flag)
  - `info` (no arguments)
- **No `--json` or `--dry-run` on apply**: Only `update` has `--dry-run`
- **`destroy` is a placeholder**: Currently just prints a message

## Hermetic Environment Considerations

The `Action::Exec` uses a hermetic environment that clears PATH. Test configs must:

- Use empty `apply` functions (no exec calls) for success cases
- Or use `ctx:write_file()` which is cross-platform and doesn't require PATH
- Avoid calling external binaries like `echo`, `touch`, etc.

## Implementation

### `crates/cli/tests/cli_smoke.rs`

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use tempfile::TempDir;

/// Get a Command for the syslua-cli binary
fn syslua_cmd() -> Command {
    Command::cargo_bin("syslua-cli").unwrap()
}

/// Create a temp directory with a config file
fn temp_config(content: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    std::fs::write(temp.path().join("init.lua"), content).unwrap();
    temp
}

/// Minimal valid config that does nothing (no exec calls)
const MINIMAL_CONFIG: &str = r#"
return {
    inputs = {},
    setup = function(_) end,
}
"#;

/// Config with a build that uses write_file (cross-platform, no PATH needed)
const BUILD_CONFIG: &str = r#"
return {
    inputs = {},
    setup = function(_)
        sys.build({
            name = "test-pkg",
            version = "1.0.0",
            apply = function(_, ctx)
                ctx:write_file(ctx.out .. "/marker.txt", "built")
                return { out = ctx.out }
            end,
        })
    end,
}
"#;

// =============================================================================
// Help & Version
// =============================================================================

#[test]
fn help_flag_works() {
    syslua_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn version_flag_works() {
    syslua_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("syslua"));
}

#[test]
fn subcommand_help_works() {
    for cmd in &["apply", "plan", "destroy", "init", "update", "info"] {
        syslua_cmd()
            .arg(cmd)
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage"));
    }
}

// =============================================================================
// init
// =============================================================================

#[test]
#[serial]
fn init_creates_config_files() {
    let temp = TempDir::new().unwrap();
    let init_dir = temp.path().join("myconfig");

    syslua_cmd()
        .arg("init")
        .arg(&init_dir)
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success();

    assert!(init_dir.join("init.lua").exists());
    assert!(init_dir.join(".luarc.json").exists());
}

#[test]
#[serial]
fn init_fails_if_config_exists() {
    let temp = temp_config(MINIMAL_CONFIG);

    syslua_cmd()
        .arg("init")
        .arg(temp.path())
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

// =============================================================================
// plan
// =============================================================================

#[test]
#[serial]
fn plan_with_minimal_config() {
    let temp = temp_config(MINIMAL_CONFIG);

    syslua_cmd()
        .arg("plan")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success();
}

#[test]
#[serial]
fn plan_with_build_shows_build_name() {
    let temp = temp_config(BUILD_CONFIG);

    syslua_cmd()
        .arg("plan")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Builds: 1"));
}

#[test]
#[serial]
fn plan_nonexistent_config_fails() {
    let temp = TempDir::new().unwrap();

    syslua_cmd()
        .arg("plan")
        .arg("/nonexistent/path/config.lua")
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .failure();
}

// =============================================================================
// apply
// =============================================================================

#[test]
#[serial]
fn apply_minimal_config() {
    let temp = temp_config(MINIMAL_CONFIG);

    syslua_cmd()
        .arg("apply")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Apply complete"));
}

#[test]
#[serial]
fn apply_creates_snapshot() {
    let temp = temp_config(BUILD_CONFIG);
    let store = temp.path().join("store");

    syslua_cmd()
        .arg("apply")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", &store)
        .assert()
        .success();

    // Verify snapshot directory was created
    assert!(store.join("snapshots").exists());
}

#[test]
#[serial]
fn apply_nonexistent_config_fails() {
    let temp = TempDir::new().unwrap();

    syslua_cmd()
        .arg("apply")
        .arg("/nonexistent/path/config.lua")
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .failure();
}

// =============================================================================
// destroy
// =============================================================================

#[test]
#[serial]
fn destroy_placeholder_works() {
    // destroy is currently a placeholder that just prints a message
    let temp = TempDir::new().unwrap();

    syslua_cmd()
        .arg("destroy")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success()
        .stdout(predicate::str::contains("destroy"));
}

// =============================================================================
// update
// =============================================================================

#[test]
#[serial]
fn update_with_no_inputs() {
    let temp = temp_config(MINIMAL_CONFIG);

    syslua_cmd()
        .arg("update")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success()
        .stdout(predicate::str::contains("up to date"));
}

#[test]
#[serial]
fn update_dry_run() {
    let temp = temp_config(MINIMAL_CONFIG);

    syslua_cmd()
        .arg("update")
        .arg(temp.path().join("init.lua"))
        .arg("--dry-run")
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));
}

// =============================================================================
// info
// =============================================================================

#[test]
#[serial]
fn info_shows_platform() {
    syslua_cmd()
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Platform"));
}

// =============================================================================
// Error Handling
// =============================================================================

#[test]
#[serial]
fn invalid_lua_syntax_fails() {
    let temp = temp_config("this is not valid lua {{{");

    syslua_cmd()
        .arg("plan")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .failure();
}

#[test]
#[serial]
fn missing_setup_function_fails() {
    let temp = temp_config("return { inputs = {} }");

    syslua_cmd()
        .arg("plan")
        .arg(temp.path().join("init.lua"))
        .env("SYSLUA_USER_STORE", temp.path().join("store"))
        .assert()
        .failure();
}
```

## Files to Create

| Path                            | Purpose                |
| ------------------------------- | ---------------------- |
| `crates/cli/tests/cli_smoke.rs` | All CLI smoke tests    |

## Files to Modify

| Path                    | Changes                                                                    |
| ----------------------- | -------------------------------------------------------------------------- |
| `crates/cli/Cargo.toml` | Add `assert_cmd`, `predicates`, `tempfile`, `serial_test` dev-dependencies |

## Testing Strategy

Run with:

```bash
cargo test -p syslua-cli
```

All tests use `#[serial]` since they set the `SYSLUA_USER_STORE` environment variable.

## Success Criteria

1. All CLI commands have at least one passing smoke test
2. Error cases return non-zero exit codes
3. Help text is verified for all commands
4. Tests are deterministic and don't flake
5. CI runs these tests on all platforms (Linux, macOS, Windows)

## Test Count

~17 smoke tests covering:
- Help & version (3 tests)
- init (2 tests)
- plan (3 tests)
- apply (3 tests)
- destroy (1 test)
- update (2 tests)
- info (1 test)
- Error handling (2 tests)

## Future Work

- Implement and test the full `destroy` command
- Add `--json` output support and tests
- Add `--dry-run` support to `apply` and test it
- Test signal handling (Ctrl+C)
- Test concurrent command execution
