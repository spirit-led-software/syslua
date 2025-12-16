//! Store operations for syslua.
//!
//! The store is the content-addressed storage for all build outputs and bind state.
//!
//! # Layout
//!
//! ```text
//! store/
//! ├── obj/                    # Build outputs (immutable, content-addressed)
//! │   └── <name>-<version>-<hash>/
//! └── bind/                   # Bind state (outputs for destroy)
//!     └── <hash>/
//!         └── state.json
//! ```

pub mod bind;

use std::path::PathBuf;

use crate::bind::BindHash;
use crate::build::BuildHash;
use crate::platform::paths::store::StorePaths;

/// Generate the store object directory name for a build.
///
/// Format: `<name>-<version>-<hash>` or `<name>-<hash>` if no version.
/// Hash is truncated to first 16 characters.
pub fn build_dir_name(name: &str, version: Option<&str>, hash: &BuildHash) -> String {
  let hash = hash.0.as_str();
  match version {
    Some(v) => format!("{}-{}-{}", name, v, hash),
    None => format!("{}-{}", name, hash),
  }
}

/// Generate the full store path for a build's output directory.
///
/// Returns the path within the system or user store based on the `system` parameter.
pub fn build_path(name: &str, version: Option<&str>, hash: &BuildHash, system: bool) -> PathBuf {
  let store = if system {
    StorePaths::system_store_path()
  } else {
    StorePaths::user_store_path()
  };
  store.join("obj").join(build_dir_name(name, version, hash))
}

/// Generate the store bind directory name for a binding.
pub fn bind_dir_name(hash: &BindHash) -> String {
  let hash = hash.0.as_str();
  hash.to_string()
}

pub fn bind_path(hash: &BindHash, system: bool) -> PathBuf {
  let store = if system {
    StorePaths::system_store_path()
  } else {
    StorePaths::user_store_path()
  };
  store.join("bind").join(bind_dir_name(hash))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn object_dir_name_with_version() {
    // 24 char hash, first 20 chars = abc123def45678901234
    let hash = BuildHash("abc123def456789012345678".to_string());
    let name = build_dir_name("ripgrep", Some("14.1.0"), &hash);
    assert_eq!(name, "ripgrep-14.1.0-abc123def45678901234");
  }

  #[test]
  fn object_dir_name_without_version() {
    let hash = BuildHash("abc123def456789012345678".to_string());
    let name = build_dir_name("my-config", None, &hash);
    assert_eq!(name, "my-config-abc123def45678901234");
  }

  #[test]
  fn object_dir_name_short_hash() {
    let hash = BuildHash("abc".to_string());
    let name = build_dir_name("test", Some("1.0"), &hash);
    assert_eq!(name, "test-1.0-abc");
  }

  #[test]
  fn object_path_includes_obj_dir() {
    let hash = BuildHash("abc123def456789012345678".to_string());
    let path = build_path("ripgrep", Some("14.1.0"), &hash, false);
    assert!(path.ends_with("store/obj/ripgrep-14.1.0-abc123def45678901234"));
  }
}
