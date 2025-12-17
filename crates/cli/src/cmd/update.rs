//! Implementation of the `sys update` command.
//!
//! This command re-resolves inputs (fetching latest revisions) and
//! updates the lock file and .luarc.json.

use anyhow::{Context, Result};

use syslua_lib::platform;
use syslua_lib::update::{UpdateOptions, find_config_path, update_inputs};

/// Execute the update command.
///
/// Re-resolves inputs by fetching the latest revisions, updates the lock file,
/// and updates .luarc.json for LuaLS IDE integration.
///
/// # Arguments
///
/// * `config` - Optional path to config file. If not provided, uses default resolution.
/// * `inputs` - Specific inputs to update. If empty, all inputs are updated.
/// * `dry_run` - If true, show what would change without making changes.
///
/// # Errors
///
/// Returns an error if the config cannot be found or input resolution fails.
pub fn cmd_update(config: Option<&str>, inputs: Vec<String>, dry_run: bool) -> Result<()> {
  let config_path = find_config_path(config).context("Failed to find config file")?;
  let system = platform::is_elevated();

  let options = UpdateOptions {
    inputs,
    dry_run,
    system,
  };

  let result = update_inputs(&config_path, &options).context("Failed to update inputs")?;

  // Print results
  if dry_run {
    println!("Dry run - no changes written");
    println!();
  }

  // Print updated inputs
  for (name, (old_rev, new_rev)) in &result.updated {
    let prefix = if dry_run { "Would update" } else { "Updated" };
    let old_short = &old_rev[..old_rev.len().min(8)];
    let new_short = &new_rev[..new_rev.len().min(8)];
    println!("  {prefix}: {name} {old_short} -> {new_short}");
  }

  // Print added inputs
  for name in &result.added {
    let prefix = if dry_run { "Would add" } else { "Added" };
    if let Some(resolved) = result.resolved.get(name) {
      let rev_short = &resolved.rev[..resolved.rev.len().min(8)];
      println!("  {prefix}: {name} ({rev_short})");
    }
  }

  // Print unchanged inputs
  if !result.unchanged.is_empty() {
    let names = result.unchanged.join(", ");
    println!("  Unchanged: {names}");
  }

  // Summary
  if result.updated.is_empty() && result.added.is_empty() {
    println!("All inputs are up to date.");
  } else if !dry_run {
    println!();
    println!(
      "Lock file updated: {}",
      config_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("syslua.lock")
        .display()
    );
  }

  Ok(())
}
