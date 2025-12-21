//! Test utilities for syslua-lib.
//!
//! This module provides cross-platform helpers for tests that need to execute
//! shell commands or use platform-specific binaries.

/// Returns the shell command and args to echo an environment variable.
///
/// Since environment variable expansion requires a shell, this returns
/// the shell binary and appropriate arguments.
#[cfg(unix)]
pub fn shell_echo_env(var: &str) -> (&'static str, Vec<String>) {
  ("/bin/sh", vec!["-c".to_string(), format!("echo \"${}\"", var)])
}

#[cfg(windows)]
pub fn shell_echo_env(var: &str) -> (&'static str, Vec<String>) {
  ("cmd.exe", vec!["/C".to_string(), format!("echo %{}%", var)])
}

/// Returns the shell command and args to execute a shell script.
#[cfg(unix)]
pub fn shell_cmd(script: &str) -> (&'static str, Vec<String>) {
  ("/bin/sh", vec!["-c".to_string(), script.to_string()])
}

#[cfg(windows)]
pub fn shell_cmd(script: &str) -> (&'static str, Vec<String>) {
  ("cmd.exe", vec!["/C".to_string(), script.to_string()])
}

/// Returns the command and args to create a marker file in the current directory.
#[cfg(unix)]
pub fn touch_file(filename: &str) -> (&'static str, Vec<String>) {
  ("/usr/bin/touch", vec![filename.to_string()])
}

#[cfg(windows)]
pub fn touch_file(filename: &str) -> (&'static str, Vec<String>) {
  // Use PowerShell to create an empty file - more reliable than cmd.exe approaches
  (
    "powershell.exe",
    vec![
      "-NoProfile".to_string(),
      "-Command".to_string(),
      format!("New-Item -ItemType File -Path '{}' -Force | Out-Null", filename),
    ],
  )
}

/// Returns the command and args to echo a message.
///
/// On Unix, this uses /bin/echo directly.
/// On Windows, echo is a shell builtin, so we wrap it in cmd.exe.
#[cfg(unix)]
pub fn echo_msg(msg: &str) -> (&'static str, Vec<String>) {
  ("/bin/echo", vec![msg.to_string()])
}

#[cfg(windows)]
pub fn echo_msg(msg: &str) -> (&'static str, Vec<String>) {
  ("cmd.exe", vec!["/C".to_string(), format!("echo {}", msg)])
}

/// Convert a path to a Lua-safe URL string.
///
/// On Windows, paths contain backslashes which become escape sequences in Lua strings.
/// This function converts backslashes to forward slashes for use in Lua code.
///
/// # Example
///
/// ```
/// use syslua_lib::util::testutil::path_to_lua_url;
/// use std::path::Path;
///
/// let path = Path::new("/tmp/my-lib");
/// let url = path_to_lua_url(path);
/// assert_eq!(url, "path:/tmp/my-lib");
/// ```
pub fn path_to_lua_url(path: &std::path::Path) -> String {
  // Convert the path to a string with forward slashes for Lua compatibility
  let path_str = path.to_string_lossy();
  // Replace backslashes with forward slashes (Windows paths)
  let normalized = path_str.replace('\\', "/");
  format!("path:{}", normalized)
}
