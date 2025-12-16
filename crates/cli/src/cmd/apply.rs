//! Implementation of the `sys apply` command.
//!
//! This command evaluates a Lua configuration file and applies changes to the system,
//! tracking state via snapshots.

use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use syslua_lib::execute::{ApplyOptions, ExecuteConfig, apply};
use syslua_lib::platform::paths;

/// Execute the apply command.
///
/// Evaluates the given Lua configuration file and applies the resulting manifest:
/// - Loads current state from snapshots
/// - Computes diff between desired and current state
/// - Destroys removed binds
/// - Realizes new builds
/// - Applies new binds
/// - Saves new snapshot
///
/// Prints a summary including counts of builds realized, binds applied/destroyed, and the snapshot ID.
pub fn cmd_apply(file: &str) -> Result<()> {
  let path = Path::new(file);

  // Determine if running as elevated
  let system = is_elevated();

  let options = ApplyOptions {
    execute: ExecuteConfig {
      parallelism: 4,
      system,
      shell: None,
    },
    system,
    dry_run: false,
  };

  // Run async apply
  let rt = tokio::runtime::Runtime::new().context("Failed to create async runtime")?;
  let result = rt.block_on(apply(path, &options)).context("Apply failed")?;

  // Print summary
  println!();
  println!("Apply complete!");
  println!("  Snapshot: {}", result.snapshot.id);
  println!("  Builds realized: {}", result.execution.realized.len());
  println!("  Builds cached: {}", result.diff.builds_cached.len());
  println!("  Binds applied: {}", result.execution.applied.len());
  println!("  Binds destroyed: {}", result.binds_destroyed);
  println!("  Binds unchanged: {}", result.diff.binds_unchanged.len());

  if !result.execution.is_success() {
    if let Some((hash, ref err)) = result.execution.build_failed {
      eprintln!("  Build failed: {} - {}", hash.0, err);
    }
    if let Some((hash, ref err)) = result.execution.bind_failed {
      eprintln!("  Bind failed: {} - {}", hash.0, err);
    }
  }

  // Print plan directory
  let base_dir = if system { paths::root_dir() } else { paths::data_dir() };
  let snapshot_path = base_dir.join("snapshots").join(format!("{}.json", result.snapshot.id));
  info!(path = %snapshot_path.display(), "snapshot saved");

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
