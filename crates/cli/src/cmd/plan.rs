//! Implementation of the `sys plan` command.
//!
//! This command evaluates a Lua configuration file and writes the resulting
//! manifest to a plan directory for later application.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use syslua_lib::consts::HASH_PREFIX_LEN;
use syslua_lib::eval::evaluate_config;
use syslua_lib::platform::paths;

/// Execute the plan command.
///
/// Evaluates the given Lua configuration file and writes the manifest to:
/// - `/syslua/plans/<hash>/manifest.json` if running as root/admin
/// - `~/.local/share/syslua/plans/<hash>/manifest.json` otherwise
///
/// Prints a summary including the plan hash, build/bind counts, and output path.
pub fn cmd_plan(file: &str) -> Result<()> {
  let path = Path::new(file);

  // Evaluate the Lua config
  let manifest = evaluate_config(path).with_context(|| format!("Failed to evaluate config: {}", file))?;

  // Compute manifest hash (truncated)
  let full_hash = manifest.compute_hash().context("Failed to compute manifest hash")?;
  let short_hash = &full_hash[..HASH_PREFIX_LEN];

  // Determine base directory based on privileges
  let base_dir = if is_elevated() {
    paths::root_dir()
  } else {
    paths::data_dir()
  };

  // Create plan directory
  let plan_dir = base_dir.join("plans").join(short_hash);
  fs::create_dir_all(&plan_dir).with_context(|| format!("Failed to create plan directory: {}", plan_dir.display()))?;

  // Write manifest as pretty-printed JSON
  let manifest_path = plan_dir.join("manifest.json");
  let manifest_json = serde_json::to_string_pretty(&manifest).context("Failed to serialize manifest")?;
  fs::write(&manifest_path, &manifest_json)
    .with_context(|| format!("Failed to write manifest: {}", manifest_path.display()))?;

  // Print summary
  println!("Plan: {}", short_hash);
  println!("Builds: {}", manifest.builds.len());
  println!("Binds: {}", manifest.bindings.len());
  println!("Path: {}", manifest_path.display());

  Ok(())
}

/// Check if the current process is running with elevated privileges.
///
/// On Unix systems, this checks if the effective user ID is root (0).
/// On Windows, this checks if the process has administrator privileges.
#[cfg(unix)]
fn is_elevated() -> bool {
  rustix::process::geteuid().is_root()
}

#[cfg(windows)]
fn is_elevated() -> bool {
  use std::mem::{size_of, zeroed};
  use windows_sys::Win32::{
    Foundation::CloseHandle,
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
  };

  unsafe {
    let mut token = 0;
    if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
      return false;
    }

    let mut elevation: TOKEN_ELEVATION = zeroed();
    let mut size: u32 = 0;
    let result = GetTokenInformation(
      token,
      TokenElevation,
      &mut elevation as *mut _ as *mut _,
      size_of::<TOKEN_ELEVATION>() as u32,
      &mut size,
    );

    CloseHandle(token);
    result != 0 && elevation.TokenIsElevated != 0
  }
}
