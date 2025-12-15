//! Configuration file evaluation.
//!
//! This module provides the `evaluate_config` function which takes a path to a
//! Lua configuration file and returns the resulting `Manifest` containing all
//! builds and bindings defined in the configuration.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use mlua::prelude::*;

use crate::lua::{loaders, runtime};
use crate::manifest::Manifest;

/// Evaluate a Lua configuration file and return the resulting manifest.
///
/// This function:
/// 1. Creates a new Lua runtime with the `sys` global
/// 2. Loads and executes the configuration file
/// 3. Returns the manifest containing all registered builds and bindings
///
/// # Arguments
/// * `path` - Path to the Lua configuration file
///
/// # Returns
/// The `Manifest` containing all builds and bindings defined in the config,
/// or a `LuaError` if evaluation fails.
///
/// # Example
/// ```ignore
/// use std::path::Path;
/// use syslua_lib::eval::evaluate_config;
///
/// let manifest = evaluate_config(Path::new("init.lua"))?;
/// println!("Builds: {}", manifest.builds.len());
/// println!("Bindings: {}", manifest.bindings.len());
/// ```
pub fn evaluate_config(path: &Path) -> LuaResult<Manifest> {
  let manifest = Rc::new(RefCell::new(Manifest::default()));

  // Create runtime and evaluate in a block to ensure lua is dropped
  // before we try to unwrap the manifest Rc
  {
    let lua = runtime::create_runtime(manifest.clone())?;
    let config = loaders::load_file_with_dir(&lua, path)?;

    // Config should return a table with { inputs, setup }
    if let LuaValue::Table(config_table) = config {
      // Get the setup function
      let setup: LuaFunction = config_table
        .get("setup")
        .map_err(|_| LuaError::external("config must return a table with a 'setup' function"))?;

      // TODO: Resolve inputs from config_table.get("inputs")
      // For now, pass an empty table
      let inputs = lua.create_table()?;

      // Call setup(inputs) to register builds and binds
      setup.call::<()>(inputs)?;
    } else {
      return Err(LuaError::external(
        "config must return a table with 'inputs' and 'setup' fields",
      ));
    }

    // lua is dropped here, releasing its references to manifest
  }

  // Now we should have the only reference to manifest
  Ok(
    Rc::try_unwrap(manifest)
      .expect("manifest still has references")
      .into_inner(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::TempDir;

  #[test]
  fn test_evaluate_empty_config() -> LuaResult<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("init.lua");
    fs::write(
      &config_path,
      r#"
        return {
          inputs = {},
          setup = function(inputs)
            -- empty setup
          end,
        }
      "#,
    )
    .unwrap();

    let manifest = evaluate_config(&config_path)?;
    assert!(manifest.builds.is_empty());
    assert!(manifest.bindings.is_empty());
    Ok(())
  }

  #[test]
  fn test_evaluate_config_with_build() -> LuaResult<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("init.lua");
    fs::write(
      &config_path,
      r#"
        return {
          inputs = {},
          setup = function(inputs)
            sys.build({
              name = "test",
              version = "1.0.0",
              apply = function(build_inputs, ctx)
                return { out = "/store/test" }
              end,
            })
          end,
        }
      "#,
    )
    .unwrap();

    let manifest = evaluate_config(&config_path)?;
    assert_eq!(manifest.builds.len(), 1);
    assert!(manifest.bindings.is_empty());

    let build = manifest.builds.values().next().unwrap();
    assert_eq!(build.name, "test");
    assert_eq!(build.version.as_deref(), Some("1.0.0"));
    Ok(())
  }

  #[test]
  fn test_evaluate_config_with_bind() -> LuaResult<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("init.lua");
    fs::write(
      &config_path,
      r#"
        return {
          inputs = {},
          setup = function(inputs)
            sys.bind({
              apply = function(bind_inputs, ctx)
                ctx:cmd({ cmd = "echo test" })
              end,
            })
          end,
        }
      "#,
    )
    .unwrap();

    let manifest = evaluate_config(&config_path)?;
    assert!(manifest.builds.is_empty());
    assert_eq!(manifest.bindings.len(), 1);
    Ok(())
  }

  #[test]
  fn test_evaluate_config_computes_stable_hash() -> LuaResult<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("init.lua");
    fs::write(
      &config_path,
      r#"
        return {
          inputs = {},
          setup = function(inputs)
            sys.build({
              name = "test",
              version = "1.0.0",
              apply = function(build_inputs, ctx)
                return { out = "/store/test" }
              end,
            })
          end,
        }
      "#,
    )
    .unwrap();

    let manifest1 = evaluate_config(&config_path)?;
    let manifest2 = evaluate_config(&config_path)?;

    let hash1 = manifest1.compute_hash().unwrap();
    let hash2 = manifest2.compute_hash().unwrap();

    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // Manifest hash is full SHA-256 (64 hex chars)
    Ok(())
  }

  #[test]
  fn test_evaluate_config_missing_setup_fails() -> LuaResult<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("init.lua");
    fs::write(
      &config_path,
      r#"
        return {
          inputs = {},
        }
      "#,
    )
    .unwrap();

    let result = evaluate_config(&config_path);
    assert!(result.is_err());
    Ok(())
  }

  #[test]
  fn test_evaluate_config_not_table_fails() -> LuaResult<()> {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("init.lua");
    fs::write(&config_path, r#"return "not a table""#).unwrap();

    let result = evaluate_config(&config_path);
    assert!(result.is_err());
    Ok(())
  }
}
