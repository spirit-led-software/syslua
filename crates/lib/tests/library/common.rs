//! Shared test helpers for library integration tests.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::prelude::*;
use syslua_lib::lua::runtime::create_runtime;
use syslua_lib::manifest::Manifest;

/// Get the workspace root directory.
fn workspace_root() -> PathBuf {
  let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  // crates/lib -> crates -> workspace root
  manifest_dir
    .parent()
    .and_then(|p| p.parent())
    .map(|p| p.to_path_buf())
    .expect("Failed to find workspace root")
}

/// Create a test runtime with the syslua library available.
///
/// Returns the Lua runtime and a reference to the manifest for inspection.
pub fn create_test_runtime() -> LuaResult<(Lua, Rc<RefCell<Manifest>>)> {
  let manifest = Rc::new(RefCell::new(Manifest::default()));
  let lua = create_runtime(manifest.clone(), false)?;

  // Ensure lua/syslua is in package.path
  // The runtime adds ./lua/?.lua but tests run from different CWD
  let syslua_path = workspace_root().join("lua");
  let syslua_path_str = syslua_path.display().to_string().replace('\\', "/");
  lua
    .load(format!(
      r#"package.path = package.path .. ";{}/?.lua;{}/?/init.lua""#,
      syslua_path_str, syslua_path_str
    ))
    .exec()?;

  Ok((lua, manifest))
}

/// Get path to a fixture file.
pub fn fixture_path(name: &str) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("fixtures")
    .join(name)
}

/// Get path to a test data file.
pub fn fixture_data_path(name: &str) -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("fixtures")
    .join("data")
    .join(name)
}
