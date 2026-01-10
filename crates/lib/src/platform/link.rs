//! Cross-platform directory linking.
//!
//! Provides symlink creation on Unix and symlink-with-junction-fallback on Windows.

use std::io;
use std::path::Path;

/// Creates a symbolic link (or junction on Windows) from `dst` pointing to `src`.
///
/// On Windows, tries symlink_dir first (requires Developer Mode), falls back to junction.
/// On Unix, creates a standard symlink.
#[cfg(windows)]
pub fn link_dir(src: &Path, dst: &Path) -> io::Result<()> {
  // Ensure parent directory exists
  if let Some(parent) = dst.parent() {
    std::fs::create_dir_all(parent)?;
  }

  // Try symlink first (requires Developer Mode or admin)
  if std::os::windows::fs::symlink_dir(src, dst).is_ok() {
    return Ok(());
  }

  // Fall back to junction (always works for directories)
  junction::create(src, dst)
}

#[cfg(not(windows))]
pub fn link_dir(src: &Path, dst: &Path) -> io::Result<()> {
  // Ensure parent directory exists
  if let Some(parent) = dst.parent() {
    std::fs::create_dir_all(parent)?;
  }

  std::os::unix::fs::symlink(src, dst)
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

  #[test]
  fn link_dir_creates_symlink() {
    let temp = tempdir().unwrap();
    let src = temp.path().join("source");
    let dst = temp.path().join("dest");

    std::fs::create_dir(&src).unwrap();
    std::fs::write(src.join("file.txt"), "content").unwrap();

    link_dir(&src, &dst).unwrap();

    assert!(dst.exists());
    assert!(dst.join("file.txt").exists());
  }

  #[test]
  fn link_dir_creates_parent_directories() {
    let temp = tempdir().unwrap();
    let src = temp.path().join("source");
    let dst = temp.path().join("nested").join("path").join("dest");

    std::fs::create_dir(&src).unwrap();

    link_dir(&src, &dst).unwrap();

    assert!(dst.exists());
  }
}
