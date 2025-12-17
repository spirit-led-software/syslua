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
  // Use copy with nul device to create empty file
  (
    "cmd.exe",
    vec!["/C".to_string(), format!("copy nul \"{}\" >nul", filename)],
  )
}

/// Path to the echo binary.
#[cfg(unix)]
pub const ECHO_BIN: &str = "/bin/echo";

/// Returns echo command and args for Windows (echo is a shell builtin).
#[cfg(windows)]
pub fn echo_with_msg(msg: &str) -> (&'static str, Vec<String>) {
  // Escape quotes in message for cmd.exe
  let escaped = msg.replace("\"", "\\\"");
  ("cmd.exe", vec!["/C".to_string(), format!("echo {}", escaped)])
}
