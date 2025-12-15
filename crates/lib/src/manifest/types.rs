//! Manifest types for syslua.
//!
//! The manifest is the central data structure that captures the complete desired
//! state of a system. It's produced by evaluating Lua configuration and contains
//! all builds and bindings to be applied.
//!
//! # Structure
//!
//! The manifest contains:
//! - `builds`: Content-addressed map of [`BuildDef`]s, keyed by [`BuildHash`]
//! - `bindings`: Content-addressed map of [`BindDef`]s, keyed by [`BindHash`]
//!
//! # Content Addressing
//!
//! Using hashes as keys provides automatic deduplication: if two different parts
//! of the configuration define identical builds, they're stored only once.
//!
//! # Serialization
//!
//! The manifest is fully serializable and can be:
//! - Stored in snapshots for state tracking
//! - Diffed against previous manifests to compute changes
//! - Hashed for quick equality checks

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bind::{BindDef, BindHash};
use crate::build::{BuildDef, BuildHash};

/// The complete desired state manifest.
///
/// This struct represents the evaluated system configuration, containing all
/// builds and bindings that should be applied.
///
/// # Content Addressing
///
/// Both maps use content-addressed hashes as keys:
/// - Enables automatic deduplication of identical definitions
/// - Makes equality checking efficient (just compare hashes)
/// - Supports incremental updates by diffing manifests
///
/// # Ordering
///
/// Uses [`BTreeMap`] to ensure deterministic serialization order, which is
/// important for reproducible manifest hashes.
///
/// # Example
///
/// ```json
/// {
///   "builds": {
///     "a1b2c3d4e5f6789012ab": { "name": "ripgrep", ... },
///     "b2c3d4e5f6789012abc1": { "name": "fd", ... }
///   },
///   "bindings": {
///     "c3d4e5f6789012abc1d2": { "apply_actions": [...], ... }
///   }
/// }
/// ```
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
  /// All builds in the manifest, keyed by their content hash.
  pub builds: BTreeMap<BuildHash, BuildDef>,
  /// All bindings in the manifest, keyed by their content hash.
  pub bindings: BTreeMap<BindHash, BindDef>,
}

impl Manifest {
  /// Compute a SHA-256 hash of the entire manifest content.
  ///
  /// The hash is computed from the JSON serialization of the manifest,
  /// providing a stable, content-based identifier for the complete state.
  ///
  /// # Use Cases
  ///
  /// - Quick equality checks between manifests
  /// - Snapshot identification
  /// - Change detection
  ///
  /// # Note
  ///
  /// Unlike [`BuildHash`] and [`BindHash`], this returns the full 64-character
  /// hash (not truncated) since it's used less frequently in paths.
  ///
  /// # Errors
  ///
  /// Returns an error if JSON serialization fails (should not happen for
  /// well-formed manifests).
  pub fn compute_hash(&self) -> Result<String, serde_json::Error> {
    let serialized = serde_json::to_string(self)?;
    let hash = Sha256::digest(serialized.as_bytes());
    Ok(format!("{:x}", hash))
  }
}
