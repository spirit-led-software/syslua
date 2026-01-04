# Agent Guidelines for syslua-lib

**Generated:** 2026-01-04 | **Commit:** c3a22f5 | **Branch:** main

## OVERVIEW

Core library for syslua. Implements content-addressed store, Lua configuration evaluation, 
atomic binds with rollback, and parallel DAG execution.

## STRUCTURE

- `action/`: Atomic execution units (Exec, FetchUrl) shared by builds/binds
- `bind/`: Mutable system state management (create/update/destroy/check)
- `build/`: Immutable content production for store
- `execute/`: DAG scheduling, parallel waves, and atomic apply orchestration
- `inputs/`: Transitive dependency resolution, lock files, namespace discovery
- `lua/`: mlua integration, global `sys` API, type conversion
- `manifest/`: Evaluated configuration IR (BTreeMap of BuildDef/BindDef)
- `platform/`: Cross-platform OS/arch abstraction (mandatory for OS APIs)
- `snapshot/`: History tracking, diffing, and rollback journal
- `util/`: Shared utilities (hash.rs for ObjectHash, placeholder.rs for Resolver)

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Parallel Execution | `execute/mod.rs` | Wave-based scheduler using JoinSet |
| Apply/Rollback Flow | `execute/apply.rs` | High-level orchestration (1.5k lines) |
| Build Hashing | `build/types.rs` | Serializable BuildDef determines ObjectHash |
| Bind Logic | `bind/execute.rs` | Platform-specific side effect application |
| Placeholder Eval | `execute/resolver.rs` | Resolves $${...} during execution |
| Transitive Deps | `inputs/resolve.rs` | Recursive input fetching (1.8k lines) |
| State Diffing | `snapshot/diff.rs` | Current vs desired state comparison |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `ExecutionDag` | struct | `execute/dag.rs` | Dependency graph for builds and binds |
| `ExecutionResolver` | struct | `execute/resolver.rs` | Resolves placeholders against completed nodes |
| `Action` | enum | `action/mod.rs` | Serializable command or fetch operation |
| `ActionCtx` | struct | `action/types.rs` | Base context for build/bind execution |
| `BindState` | struct | `bind/state.rs` | Persisted outputs for drift check/destroy |
| `StateDiff` | struct | `snapshot/diff.rs` | Comparison between current and desired state |
| `LuaNamespace` | struct | `inputs/types.rs` | Discovered Lua module paths from inputs |
| `ObjectHash` | struct | `util/hash.rs` | 20-char truncated SHA256 |
| `Resolver` | trait | `placeholder.rs` | JIT placeholder substitution |

## CONVENTIONS

- **Error Policy**: 18+ module-specific error enums using `thiserror`. All errors must be serializable.
- **Placeholder Resolution**: Resolved ONLY during execution via `ExecutionResolver`. Never store resolved values in `Def`.
- **Deterministic IR**: Use `BTreeMap` for all serializable maps to ensure stable hashes.
- **Bind ID**: IDs required for `update()` support; anonymous binds only support create/destroy.
- **Out Directory**: Builds must use `ctx:out()` placeholder for all filesystem output.
- **Store Layout**: `build/<hash>/` for immutable content, `bind/<hash>/` for state tracking.
- **Module Hierarchy**: execute → manifest → {build,bind} → action → util/hash (one-way deps).

## ANTI-PATTERNS

- **Mutable Global State**: Use `ActionCtx` and `ExecutionResolver` to pass state.
- **Direct FS in Build**: Builds MUST remain pure; use `Exec` actions for all changes.
- **HashMap in Defs**: Breaking deterministic hashing/serialization.
- **Unresolved Placeholders**: Accessing $${...} strings without passing through a resolver.
- **Builds referencing binds**: Dependency is one-way only.

## COMPLEXITY HOTSPOTS

| File | Lines | Complexity |
|------|-------|------------|
| `inputs/resolve.rs` | 1860 | Transitive resolution loop, override handling |
| `execute/apply.rs` | 1593 | Atomic rollback, snapshot management |
| `execute/mod.rs` | 1234 | Wave parallelism with JoinSet |
| `bind/execute.rs` | 1128 | Four Resolver types, platform dispatch |
| `execute/dag.rs` | 1071 | Heterogeneous build/bind graph |
