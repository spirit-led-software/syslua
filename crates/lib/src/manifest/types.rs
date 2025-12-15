use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bind::{BindDef, BindHash};
use crate::build::{BuildDef, BuildHash};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
  pub builds: BTreeMap<BuildHash, BuildDef>,
  pub bindings: BTreeMap<BindHash, BindDef>,
}

impl Manifest {
  /// Compute a SHA-256 hash of the manifest content.
  ///
  /// The hash is computed from the JSON serialization of the manifest,
  /// providing a stable, content-based identifier.
  pub fn compute_hash(&self) -> Result<String, serde_json::Error> {
    let serialized = serde_json::to_string(self)?;
    let hash = Sha256::digest(serialized.as_bytes());
    Ok(format!("{:x}", hash))
  }
}
