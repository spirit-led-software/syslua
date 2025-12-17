//! Cmd action implementation.
//!
//! This module handles executing shell commands with isolated environments,
//! following Nix-inspired principles.

use std::collections::BTreeMap;
use std::path::Path;

use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, info};

use crate::execute::types::ExecuteError;

/// Options for executing a shell command in a build.
///
/// This is a builder-pattern struct for configuring [`Action::Cmd`] actions.
/// It can be constructed from a string slice for simple commands.
///
/// # Example
///
/// ```ignore
/// // Simple command
/// ctx.cmd("make install");
///
/// // With environment and working directory
/// ctx.cmd(
///     BuildCmdOptions::new("make")
///         .with_args(vec!["install".to_string()])
///         .with_env(env)
///         .with_cwd("/build")
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CmdOpts {
  /// The command string to execute.
  pub cmd: String,
  /// Optional arguments for the command.
  pub args: Option<Vec<String>>,
  /// Optional environment variables to set.
  pub env: Option<BTreeMap<String, String>>,
  /// Optional working directory.
  pub cwd: Option<String>,
}

impl CmdOpts {
  /// Create a new command with default options.
  pub fn new(cmd: &str) -> Self {
    Self {
      cmd: cmd.to_string(),
      args: None,
      env: None,
      cwd: None,
    }
  }

  /// Set arguments for the command.
  pub fn with_args(mut self, args: Vec<String>) -> Self {
    self.args = Some(args);
    self
  }

  /// Set environment variables for the command.
  pub fn with_env(mut self, env: BTreeMap<String, String>) -> Self {
    self.env = Some(env);
    self
  }

  /// Set the working directory for the command.
  pub fn with_cwd(mut self, cwd: &str) -> Self {
    self.cwd = Some(cwd.to_string());
    self
  }
}

impl From<&str> for CmdOpts {
  fn from(cmd: &str) -> Self {
    CmdOpts::new(cmd)
  }
}

pub fn parse_cmd_opts(opts: LuaValue) -> LuaResult<CmdOpts> {
  match opts {
    LuaValue::String(s) => {
      let cmd = s.to_str()?.to_string();
      Ok(CmdOpts::new(&cmd))
    }
    LuaValue::Table(table) => {
      let cmd: String = table.get("cmd")?;
      let args: Option<Vec<String>> = table.get("args")?;
      let cwd: Option<String> = table.get("cwd")?;
      let env: Option<LuaTable> = table.get("env")?;

      let mut opts = CmdOpts::new(&cmd);

      let mut args_vec = Vec::new();
      if let Some(a) = args {
        args_vec = a;
      }
      opts = opts.with_args(args_vec);

      if let Some(cwd) = cwd {
        opts = opts.with_cwd(&cwd);
      }

      if let Some(env_table) = env {
        let mut env_map = BTreeMap::new();
        for pair in env_table.pairs::<String, String>() {
          let (key, value) = pair?;
          env_map.insert(key, value);
        }
        opts = opts.with_env(env_map);
      }
      Ok(opts)
    }
    _ => Err(LuaError::external("cmd() expects a string or table with 'cmd' field")),
  }
}

/// Execute a Cmd action.
///
/// Runs the command in an isolated environment:
/// - Clears all environment variables
/// - On Windows, preserves critical system vars (SystemRoot, SYSTEMDRIVE, WINDIR, COMSPEC, PATHEXT)
/// - Sets PATH to /path-not-set (C:\path-not-set on Windows) to fail fast if deps aren't specified
/// - Sets HOME to /homeless-shelter
/// - Sets TMPDIR/TMP/TEMP/TEMPDIR to a temp directory within out_dir
/// - Sets `out` to the output directory
/// - Merges user-specified environment variables
///
/// # Arguments
///
/// * `opts` - The command options to execute
/// * `out_dir` - The build's output directory
///
/// # Returns
///
/// The stdout of the command on success (trimmed).
pub async fn execute_cmd(
  cmd: &str,
  args: Option<&Vec<String>>,
  env: Option<&BTreeMap<String, String>>,
  cwd: Option<&str>,
  out_dir: &Path,
) -> Result<String, ExecuteError> {
  info!(cmd = %cmd, "executing command");

  // Create temp directory for the build
  let tmp_dir = out_dir.join("tmp");
  tokio::fs::create_dir_all(&tmp_dir).await?;

  let working_dir = cwd.map(Path::new).unwrap_or(out_dir);

  // Build the command with isolated environment
  let mut command = Command::new(cmd);
  command
    .args(args.unwrap_or(&Vec::new()))
    .current_dir(working_dir)
    // Clear all environment variables
    .env_clear();

  // On Windows, preserve critical system variables required for shell startup.
  // Unlike Unix, Windows shells (especially PowerShell) require certain system
  // environment variables to locate DLLs and resolve executables.
  #[cfg(windows)]
  {
    for var in ["SystemRoot", "SYSTEMDRIVE", "WINDIR", "COMSPEC", "PATHEXT"] {
      if let Ok(val) = std::env::var(var) {
        command.env(var, val);
      }
    }
  }

  // Set platform-appropriate isolated PATH
  #[cfg(unix)]
  command.env("PATH", "/path-not-set");
  #[cfg(windows)]
  {
    let system_drive = std::env::var("SYSTEMDRIVE").unwrap_or_else(|_| "C:".to_string());
    command.env("PATH", format!("{}\\path-not-set", system_drive));
  }

  // Set isolated environment (cross-platform)
  command
    .env("HOME", "/homeless-shelter")
    .env("TMPDIR", &tmp_dir)
    .env("TMP", &tmp_dir)
    .env("TEMP", &tmp_dir)
    .env("TEMPDIR", &tmp_dir)
    .env("out", out_dir)
    // Set a minimal locale
    .env("LANG", "C")
    .env("LC_ALL", "C")
    // Set SOURCE_DATE_EPOCH for reproducible timestamps
    // Value is 315532800 = January 1, 1980 00:00:00 UTC (ZIP epoch)
    .env("SOURCE_DATE_EPOCH", "315532800");

  // Merge user-specified environment variables
  if let Some(user_env) = env {
    for (key, value) in user_env {
      command.env(key, value);
    }
  }

  debug!(cmd = %cmd,  working_dir = ?working_dir, "spawning process");

  let output = command.output().await?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Log output for debugging
    if !stderr.is_empty() {
      debug!(stderr = %stderr, "command stderr");
    }
    if !stdout.is_empty() {
      debug!(stdout = %stdout, "command stdout");
    }

    return Err(ExecuteError::CmdFailed {
      cmd: cmd.to_string(),
      code: output.status.code(),
    });
  }

  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

  if !stdout.is_empty() {
    debug!(stdout = %stdout, "command output");
  }

  Ok(stdout)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::util::testutil::{ECHO_BIN, shell_cmd, shell_echo_env, touch_file};
  use tempfile::TempDir;

  #[tokio::test]
  async fn execute_simple_command() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let result = execute_cmd(ECHO_BIN, Some(&vec!["hello".to_string()]), None, None, out_dir)
      .await
      .unwrap();

    assert_eq!(result, "hello");
  }

  #[tokio::test]
  async fn execute_command_with_env() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let mut env = BTreeMap::new();
    env.insert("MY_VAR".to_string(), "my_value".to_string());

    let (cmd, args) = shell_echo_env("MY_VAR");
    let result = execute_cmd(cmd, Some(&args), Some(&env), None, out_dir).await.unwrap();

    assert_eq!(result, "my_value");
  }

  #[tokio::test]
  async fn execute_command_out_env_set() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let (cmd, args) = shell_echo_env("out");
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    assert_eq!(result, out_dir.to_string_lossy());
  }

  #[tokio::test]
  async fn execute_command_isolated_path() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let (cmd, args) = shell_echo_env("PATH");
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    #[cfg(unix)]
    assert_eq!(result, "/path-not-set");
    #[cfg(windows)]
    {
      let system_drive = std::env::var("SYSTEMDRIVE").unwrap_or_else(|_| "C:".to_string());
      assert_eq!(result, format!("{}\\path-not-set", system_drive));
    }
  }

  /// On Windows, critical system variables must be preserved for cmd.exe to function.
  #[tokio::test]
  #[cfg(windows)]
  async fn execute_command_preserves_windows_system_vars() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    // SystemRoot should be preserved for Windows to function properly
    let (cmd, args) = shell_echo_env("SystemRoot");
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    // SystemRoot is typically C:\Windows or similar
    assert!(!result.is_empty(), "SystemRoot should be preserved");
    assert!(
      result.to_lowercase().contains("windows"),
      "SystemRoot should contain 'Windows', got: {}",
      result
    );
  }

  #[tokio::test]
  async fn execute_command_has_source_date_epoch() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let (cmd, args) = shell_echo_env("SOURCE_DATE_EPOCH");
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    assert_eq!(result, "315532800");
  }

  #[tokio::test]
  async fn execute_command_failure() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let (cmd, args) = shell_cmd("exit 1");
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await;

    assert!(matches!(result, Err(ExecuteError::CmdFailed { code: Some(1), .. })));
  }

  #[tokio::test]
  async fn execute_command_with_cwd() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    // Create a subdirectory
    let sub_dir = out_dir.join("subdir");
    tokio::fs::create_dir(&sub_dir).await.unwrap();

    // Run a command that creates a marker file in the cwd
    let (cmd, args) = touch_file("cwd_marker");
    execute_cmd(cmd, Some(&args), None, Some(sub_dir.to_str().unwrap()), out_dir)
      .await
      .unwrap();

    // Verify the marker file was created in the subdirectory (proving cwd was set correctly)
    assert!(
      sub_dir.join("cwd_marker").exists(),
      "cwd_marker should exist in subdirectory, proving cwd was set correctly"
    );
  }

  #[tokio::test]
  async fn execute_command_creates_tmp_dir() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let (cmd, args) = shell_echo_env("TMPDIR");
    execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    // Verify tmp directory was created
    assert!(out_dir.join("tmp").exists());
  }

  #[tokio::test]
  #[cfg(unix)]
  async fn execute_multiline_command() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    let script = r#"
      x=1
      y=2
      echo $((x + y))
    "#;

    let (cmd, args) = shell_cmd(script);
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    assert_eq!(result, "3");
  }

  #[tokio::test]
  #[cfg(windows)]
  async fn execute_multiline_command() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();

    // Test command chaining with cmd.exe using && operator
    // (cmd.exe doesn't execute multiple lines like Unix shells)
    let script = "echo first && echo 3";

    let (cmd, args) = shell_cmd(script);
    let result = execute_cmd(cmd, Some(&args), None, None, out_dir).await.unwrap();

    // cmd.exe should execute both commands, output ends with "3"
    assert!(
      result.ends_with("3"),
      "Expected output to end with '3', got: {}",
      result
    );
  }
}
