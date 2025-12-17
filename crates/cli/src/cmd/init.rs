//! Implementation of the `sys init` command.
//!
//! This command initializes a new syslua configuration directory with
//! template files and sets up the store structure.

use std::path::Path;

use anyhow::{Context, Result};

use syslua_lib::init::{InitOptions, init};
use syslua_lib::platform;

/// Execute the init command.
///
/// Initializes a new syslua configuration directory at the given path with:
/// - `init.lua` entry point with examples
/// - `.luarc.json` for LuaLS IDE integration
/// - Store structure and type definitions
///
/// # Errors
///
/// Returns an error if files already exist or if there are permission issues.
pub fn cmd_init(path: &str) -> Result<()> {
  let config_path = Path::new(path);
  let system = platform::is_elevated();

  let options = InitOptions {
    config_path: config_path.to_path_buf(),
    system,
  };

  let result = init(&options).context("Failed to initialize configuration")?;

  println!("Initialized syslua configuration!");
  println!();
  println!("  Config directory: {}", result.config_dir.display());
  println!("  Entry point:      {}", result.init_lua.display());
  println!("  LuaLS config:     {}", result.luarc_json.display());
  println!("  Type definitions: {}", result.types_dir.display());
  println!("  Store:            {}", result.store_dir.display());
  println!();
  println!("Next steps:");
  println!("  1. Edit {} to configure your system", result.init_lua.display());
  println!("  2. Run: sys apply {}", result.config_dir.display());

  Ok(())
}
