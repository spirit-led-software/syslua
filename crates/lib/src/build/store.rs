use std::path::PathBuf;

use crate::{store::paths::StorePaths, util::hash::ObjectHash};

/// Generate the store object directory name for a build.
pub fn build_dir_name(hash: &ObjectHash) -> String {
  let hash = hash.0.as_str();
  hash.to_string()
}

/// Generate the full store path for a build's output directory.
///
/// Returns the path within the system or user store based on the `system` parameter.
pub fn build_path(hash: &ObjectHash, system: bool) -> PathBuf {
  let store = if system {
    StorePaths::system_store_path()
  } else {
    StorePaths::user_store_path()
  };
  store.join("obj").join(build_dir_name(hash))
}

#[cfg(test)]
mod tests {
  use crate::util::hash::ObjectHash;

  use super::*;

  #[test]
  fn object_dir_name() {
    let hash = ObjectHash("abc123def45678901234".to_string());
    let name = build_dir_name(&hash);
    assert_eq!(name, "abc123def45678901234");
  }

  #[test]
  fn object_path_includes_obj_dir() {
    use std::path::Path;

    let hash = ObjectHash("abc123def45678901234".to_string());
    let path = build_path(&hash, false);
    // Check that path ends with obj/name-version-{hash}
    // Note: We don't check for "store" because SYSLUA_USER_STORE env var can override to any path
    let expected_suffix = Path::new("obj").join("abc123def45678901234");
    assert!(
      path.ends_with(&expected_suffix),
      "Path {:?} should end with {:?}",
      path,
      expected_suffix
    );
  }
}
