use std::path::PathBuf;

use crate::{store::paths::StorePaths, util::hash::ObjectHash};

/// Generate the store bind directory name for a binding.
pub fn bind_dir_name(hash: &ObjectHash) -> String {
  let hash = hash.0.as_str();
  hash.to_string()
}

pub fn bind_dir_path(hash: &ObjectHash, system: bool) -> PathBuf {
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
  fn test_bind_dir_name() {
    let hash = ObjectHash("abc123def45678901234".to_string());
    let name = bind_dir_name(&hash);
    assert_eq!(name, "abc123def45678901234");
  }

  #[test]
  fn test_bind_path_includes_bind_dir() {
    use std::path::Path;

    let hash = ObjectHash("abc123def45678901234".to_string());
    let path = bind_dir_path(&hash, false);
    // Check that path ends with bind/{hash}
    // Note: We don't check for "store" because SYSLUA_USER_STORE env var can override to any path
    let expected_suffix = Path::new("bind").join("abc123def45678901234");
    assert!(
      path.ends_with(&expected_suffix),
      "Path {:?} should end with {:?}",
      path,
      expected_suffix
    );
  }
}
