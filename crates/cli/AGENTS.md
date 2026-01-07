# Agent Guidelines for syslua-cli

**Generated:** 2026-01-04 | **Commit:** c3a22f5 | **Branch:** main

## OVERVIEW

Binary crate `sys` - command-line interface for syslua. Thin layer over `syslua-lib`.

## STRUCTURE

```
cli/
├── src/
│   ├── main.rs      # Entry point, clap CLI definition, logging setup
│   ├── cmd/         # One file per command (apply, destroy, diff, gc, info, init, plan, snapshot, status, update)
│   ├── output.rs    # OutputFormat enum (text/json)
│   └── prompts.rs   # Interactive prompts
└── tests/
    ├── integration/ # CLI integration tests (assert_cmd)
    └── fixtures/    # Test Lua configs
```

## COMMANDS

| Command        | File         | Purpose                                   |
| -------------- | ------------ | ----------------------------------------- |
| `sys apply`    | `apply.rs`   | Evaluate config, apply changes            |
| `sys plan`     | `plan.rs`    | Dry-run of apply                          |
| `sys destroy`  | `destroy.rs` | Remove all binds                          |
| `sys diff`     | `diff.rs`    | Compare snapshots                         |
| `sys update`   | `update.rs`  | Re-resolve inputs to latest               |
| `sys status`   | `status.rs`  | Current state vs expected                 |
| `sys gc`       | `gc.rs`      | Clean unused store objects                |
| `sys info`     | `info.rs`    | Display system info                       |
| `sys init`     | `init.rs`    | Initialize config directory               |
| `sys snapshot` | `snapshot/`  | Subcommands: list, show, rollback, delete |

## ADDING A COMMAND

1. Create `src/cmd/<name>.rs` with `pub fn cmd_<name>(...) -> anyhow::Result<()>`
2. Add module to `src/cmd/mod.rs` and export the function
3. Add variant to `Commands` enum in `main.rs`
4. Add match arm in `main()` to call the function

## CONVENTIONS

- **Error handling**: Use `anyhow::Result` for all command functions
- **Output**: Support `--output text|json` via `OutputFormat` enum
- **Logging**: Use global `--log-level` and `--log-format` flags
- **Color**: Respect `--color auto|always|never` via `owo-colors`
- **Tokio**: Commands use `#[tokio::main]` via the runtime in main

## TESTING

```bash
cargo test -p syslua-cli                    # All CLI tests
cargo test -p syslua-cli --test integration # Integration only
```

- Tests use `assert_cmd` + `predicates` for CLI assertions
- `serial_test` for tests touching global state
- Fixtures in `tests/fixtures/*.lua`

## ANTI-PATTERNS

- **Direct lib internals**: Use `syslua_lib` public API only
- **Hardcoded paths**: Use `platform::paths` from lib
- **Blocking in async**: All I/O through tokio
