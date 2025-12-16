//! Snapshot management for syslua.
//!
//! Snapshots capture system state as a manifest of builds and binds.
//! They enable rollback, diff computation, and garbage collection.
//!
//! # Modules
//!
//! - [`types`]: Core types (`Snapshot`, `SnapshotIndex`, etc.)
//! - [`storage`]: Disk persistence (`SnapshotStore`)
//! - [`diff`]: Diff computation between manifests

mod diff;
mod storage;
mod types;

pub use diff::*;
pub use storage::*;
pub use types::*;
