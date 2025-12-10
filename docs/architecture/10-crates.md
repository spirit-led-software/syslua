# Crate Structure

> Part of the [sys.lua Architecture](./00-overview.md) documentation.

This document covers the Rust crate structure and dependencies.

## Directory Structure

```
sys.lua/
├── crates/
│   ├── cli/       # CLI application
│   ├── core/      # Core logic: store, inputs, snapshots, build
│   ├── lua/       # Lua config parsing and module system
│   ├── platform/  # OS-specific functionality (services, env, paths)
│   └── sops/      # SOPS integration for secrets
├── lib/           # Standard library modules (Lua)
├── pkgs/          # Package definitions (Lua files)
├── modules/       # Reusable module definitions (Lua files)
├── examples/      # Example configurations
└── docs/          # Documentation
```

## sys-cli

The command-line interface. Provides commands for applying configs, managing packages, and system introspection.

### Commands

| Command                  | Purpose                                            |
| ------------------------ | -------------------------------------------------- |
| `apply [init.lua]`       | Apply a configuration file (declarative)           |
| `plan [init.lua]`        | Dry-run showing what changes would be made         |
| `status`                 | Show current environment status                    |
| `list`                   | List installed packages                            |
| `history`                | Show snapshot history with details                 |
| `rollback [snapshot_id]` | Rollback to a previous snapshot                    |
| `gc`                     | Garbage collect orphaned objects from store        |
| `update [input]`         | Update lock file inputs (all or specific)          |
| `shell`                  | Enter project environment or ephemeral shell       |
| `env`                    | Print environment activation script                |
| `secrets rotate`         | Re-encrypt secrets with new keys                   |
| `secrets set <key>`      | Set a secret value                                 |
| `completions <shell>`    | Generate shell completions                         |

### Shell Completions

```bash
# Generate completions
$ sys completions bash
$ sys completions zsh
$ sys completions fish
$ sys completions powershell

# Install for bash
$ sys completions bash > ~/.local/share/bash-completion/completions/sys

# Install for zsh
$ sys completions zsh > ~/.local/share/zsh/site-functions/_sys
```

### CLI Dependencies

| Crate           | Version   | Purpose                                       |
| --------------- | --------- | --------------------------------------------- |
| `clap`          | 4.5       | Command-line argument parsing                 |
| `clap_complete` | 4.5       | Shell completion generation                   |
| `console`       | 0.16      | Terminal colors, progress bars                |
| `indicatif`     | 0.18      | Progress bars for downloads                   |
| `dialoguer`     | 0.12      | Interactive prompts and confirmations         |
| `atty`          | 0.2       | Detect if running in TTY                      |
| `sys-core`      | workspace | Core logic (internal)                         |
| `sys-platform`  | workspace | Platform abstractions (internal)              |

## sys-core

Core functionality shared across CLI and agent.

### Modules

| Module       | Purpose                                          |
| ------------ | ------------------------------------------------ |
| `derivation` | Derivation evaluation, hashing, and build context |
| `manifest`   | Manifest data structures and validation          |
| `store`      | Derivation realization, object storage, GC       |
| `priority`   | Priority-based conflict resolution               |
| `merge`      | Declaration merging for singular/mergeable values |
| `dag`        | Execution DAG construction and sorting           |
| `inputs`     | Input resolution, lock file management           |
| `snapshot`   | State tracking, rollback support                 |
| `plan`       | Diff computation between manifest and state      |
| `executor`   | DAG execution engine with parallel support       |
| `service`    | Cross-platform service management                |
| `secrets`    | SOPS integration for secrets management          |
| `env`        | Environment script generation                    |
| `activation` | Activation script hooks                          |
| `types`      | Shared data structures                           |
| `error`      | Error types                                      |

### Core Dependencies

| Crate          | Version   | Purpose                                  |
| -------------- | --------- | ---------------------------------------- |
| `mlua`         | 0.11      | Lua runtime integration                  |
| `reqwest`      | 0.12      | HTTP client for fetchUrl                 |
| `gix`          | 0.75      | Git operations for fetchGit              |
| `sha2`         | 0.10      | SHA-256 hashing for content addressing   |
| `hex`          | 0.4       | Hex encoding/decoding for hashes         |
| `tar`          | 0.4       | Extract .tar.gz archives                 |
| `flate2`       | 1.0       | Gzip compression/decompression           |
| `zip`          | 6.0       | Extract .zip archives (Windows releases) |
| `walkdir`      | 2.5       | Recursive directory traversal            |
| `tempfile`     | 3.10      | Temporary directories for downloads      |
| `semver`       | 1.0       | Semantic version parsing                 |
| `petgraph`     | 0.8       | DAG construction and topological sorting |
| `rayon`        | 1.10      | Parallel execution of DAG nodes          |
| `toml`         | 0.9       | TOML parsing for lock files              |
| `sys-lua`      | workspace | Lua integration (internal)               |
| `sys-platform` | workspace | Platform abstractions (internal)         |
| `sys-sops`     | workspace | SOPS integration (internal)              |

## sys-lua

Lua integration using the `mlua` crate.

### Responsibilities

- Parse user config files (`init.lua`)
- Parse registry package definitions (`registry/*.lua`)
- Provide Lua APIs at multiple abstraction levels
- Evaluate configuration and produce a declarative manifest

### Lua Dependencies

| Crate   | Version | Purpose                            |
| ------- | ------- | ---------------------------------- |
| `mlua`  | 0.11    | Lua 5.4 runtime with safe bindings |
| `serde` | 1.0     | Convert Lua tables to Rust structs |

**mlua features enabled:**

- `lua54` - Use Lua 5.4 (latest stable)
- `serialize` - Serde integration for Lua tables
- `async` - Async function support for HTTP/Git operations
- `vendored` - Bundle Lua to avoid system dependency

## sys-platform

Platform-specific functionality for OS detection, paths, services, and environment variables.

### Platform Dependencies

| Crate     | Version | Purpose                                |
| --------- | ------- | -------------------------------------- |
| `dirs`    | 6.0     | Standard directory paths               |
| `whoami`  | 1.5     | User/system information                |
| `libc`    | 0.2     | Unix system calls (chattr, chflags)    |
| `winapi`  | 0.3     | Windows API bindings (ACLs, registry)  |
| `nix`     | 0.30    | Unix/POSIX system APIs                 |
| `sysinfo` | 0.37    | System information (OS, architecture)  |

**Platform-specific features:**

- Linux: `libc`, `nix` for immutability flags, systemd service management
- macOS: `libc`, `nix` for immutability flags, launchd service management
- Windows: `winapi` for ACLs, registry, Windows services

## sys-sops

SOPS integration for encrypted secrets management.

### SOPS Dependencies

| Crate    | Version | Purpose                      |
| -------- | ------- | ---------------------------- |
| `age`    | 0.11    | Age encryption/decryption    |
| `base64` | 0.22    | Base64 encoding              |

**Features:**

- `default` = `["age"]` - Age encryption enabled by default

**Note:** SOPS file format handling is implemented in Rust rather than shelling out to the `sops` binary. Only Age encryption is supported (pure Rust, no system dependencies). GPG support is not included.

## Shared Dependencies (Workspace-level)

| Crate                | Version | Purpose                                 |
| -------------------- | ------- | --------------------------------------- |
| `mlua`               | 0.11    | Lua 5.4 runtime                         |
| `serde`              | 1.0     | Serialization/deserialization           |
| `serde_json`         | 1.0     | JSON support                            |
| `serde_yaml`         | 0.9     | YAML support for SOPS secrets           |
| `thiserror`          | 2.0     | Error type derivation                   |
| `anyhow`             | 1.0     | Flexible error handling                 |
| `tracing`            | 0.1     | Structured logging                      |
| `tracing-subscriber` | 0.3     | Log formatting and filtering            |
| `tokio`              | 1.0     | Async runtime for HTTP, Git, parallel   |

## Development Dependencies

| Crate        | Version | Purpose                              |
| ------------ | ------- | ------------------------------------ |
| `tempfile`   | 3.10    | Temporary directories for tests      |
| `assert_cmd` | 2.0     | CLI testing utilities                |
| `predicates` | 3.1     | Assertions for CLI output            |
| `mockito`    | 1.4     | HTTP mock server                     |
| `proptest`   | 1.4     | Property-based testing               |

## Dependency Selection Criteria

All dependencies were selected based on:

1. **Maturity**: Version 1.0+ or widely adopted
2. **Maintenance**: Active development with recent releases
3. **Documentation**: Comprehensive docs and examples
4. **Community**: Large user base and ecosystem support
5. **Cross-platform**: Works on Linux, macOS, and Windows
6. **Performance**: Efficient implementation
7. **Security**: Regular security audits

## See Also

- [Overview](./00-overview.md) - High-level architecture
- [Derivations](./01-derivations.md) - Core derivation system
- [Platform](./09-platform.md) - Platform-specific details
