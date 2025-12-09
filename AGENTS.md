# Agent Guidelines for sys.lua

- Build: `cargo build` (workspace), `cargo build -p sys-cli` for CLI.
- Test: `cargo test` (all), `cargo test -p <crate> <filter>` for a single test or module; append `-- --nocapture` when debugging.
- Lint/format: run `cargo fmt` and `cargo clippy --all-targets --all-features` before proposing non-trivial changes.
- Use Rust 2021 idioms; `snake_case` for functions/locals, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts; avoid one-letter names except for short loops.
- Imports: group `std`, then external crates, then internal modules; prefer explicit imports over glob (`*`) where reasonable.
- Error handling: use `Result` and existing error enums; propagate with `?`; prefer descriptive variants/messages over `.unwrap()`/`.expect()` except in clearly unreachable cases.
- Types: be explicit at public boundaries; prefer references (`&str`, slices) over owned types when clones are not required; keep manifests/DAG types consistent with `ARCHITECTURE.md`.
- Side effects: keep hooks/build steps deterministic and sandbox-friendly; avoid hidden I/O or network access outside the flows described in `ARCHITECTURE.md`.
- Lua config examples should follow `config = function(opts) ... end`, use `syslua.lib` helpers, and keep options typed and validated as documented.
- When adding CLI commands, route logic through `sys-core` instead of duplicating behavior in `sys-cli`.
- Prefer extending existing module/option systems to adding ad-hoc flags or environment variables.
- For cross-platform behavior, rely on `sys-platform` abstractions instead of OS-specific APIs where possible.
- Logging: use existing logging facilities and levels; keep messages actionable and avoid excessive default TRACE-level noise.
- Tests: favor fast, deterministic unit tests per crate; for integration flows, mimic `sys apply/plan` behavior with targeted cases rather than broad end-to-end scripts.
- There are currently no Cursor rules (`.cursor/rules/` or `.cursorrules`) or Copilot rules (`.github/copilot-instructions.md`); if they are added later, update this file to reference them.
- Reference [ARCHITECTURE.md](./ARCHITECTURE.md) for high-level design principles and module interactions.
