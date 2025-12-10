//! Build context for derivation builds
//!
//! The BuildContext provides helpers for:
//! - Fetching URLs
//! - Extracting archives
//! - File operations (copy, move, write, chmod, symlink)
//! - Running shell commands

use crate::Result;
use crate::error::CoreError;
use crate::store::sha256_file;
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;
use tracing::{debug, info, trace};

/// Build context provided to derivation build functions.
///
/// Provides helpers for fetching, filesystem operations, and shell execution.
#[derive(Debug)]
pub struct BuildContext {
    /// Primary output directory path
    pub out: PathBuf,

    /// Map of output names to paths (for multi-output derivations)
    pub outputs: HashMap<String, PathBuf>,

    /// Mutable environment variables for ctx.run
    pub env: HashMap<String, String>,

    /// Temporary directory for intermediate files
    temp_dir: PathBuf,
}

impl BuildContext {
    /// Create a new build context.
    ///
    /// - `out_dir`: The primary output directory
    /// - `temp_dir`: A temporary directory for intermediate files
    pub fn new(out_dir: PathBuf, temp_dir: PathBuf) -> Self {
        let mut outputs = HashMap::new();
        outputs.insert("out".to_string(), out_dir.clone());

        // Initialize basic environment
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

        Self {
            out: out_dir,
            outputs,
            env,
            temp_dir,
        }
    }

    /// Add an additional output.
    pub fn add_output(&mut self, name: &str, path: PathBuf) {
        self.outputs.insert(name.to_string(), path);
    }

    // ========== Fetch Operations ==========

    /// Fetch a URL and verify its SHA-256 hash.
    ///
    /// Returns the path to the downloaded file.
    pub fn fetch_url(&self, url: &str, sha256: &str) -> Result<PathBuf> {
        info!("Fetching URL: {}", url);

        // Determine filename from URL
        let filename = url.rsplit('/').next().unwrap_or("download");
        let download_path = self.temp_dir.join(filename);

        // Download the file
        self.download_file(url, &download_path)?;

        // Verify hash
        let actual_hash = sha256_file(&download_path)?;
        if actual_hash != sha256 {
            return Err(CoreError::HashMismatch {
                expected: sha256.to_string(),
                actual: actual_hash,
            });
        }

        debug!("Hash verified: {}", sha256);
        Ok(download_path)
    }

    /// Download a file from a URL (internal helper).
    fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        // Use curl/wget for simplicity in this implementation
        // In production, we'd use reqwest with proper async handling
        #[cfg(unix)]
        {
            let status = Command::new("curl")
                .args(["-fsSL", "-o"])
                .arg(dest)
                .arg(url)
                .status()?;

            if !status.success() {
                return Err(CoreError::FetchFailed {
                    url: url.to_string(),
                    message: format!("curl exited with status: {}", status),
                });
            }
        }

        #[cfg(windows)]
        {
            let status = Command::new("powershell")
                .args([
                    "-Command",
                    &format!(
                        "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                        url,
                        dest.display()
                    ),
                ])
                .status()?;

            if !status.success() {
                return Err(CoreError::FetchFailed {
                    url: url.to_string(),
                    message: format!("PowerShell download failed with status: {}", status),
                });
            }
        }

        Ok(())
    }

    // ========== Archive Operations ==========

    /// Unpack an archive to a destination directory.
    ///
    /// Supports: .tar.gz, .tgz, .tar, .zip
    ///
    /// If `dest` is None, unpacks to `ctx.out`.
    pub fn unpack(&self, archive: &Path, dest: Option<&Path>) -> Result<PathBuf> {
        let dest = dest.unwrap_or(&self.out);
        info!("Unpacking {} to {}", archive.display(), dest.display());

        fs::create_dir_all(dest)?;

        let filename = archive.file_name().and_then(|f| f.to_str()).unwrap_or("");

        if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
            self.unpack_tar_gz(archive, dest)?;
        } else if filename.ends_with(".tar") {
            self.unpack_tar(archive, dest)?;
        } else if filename.ends_with(".zip") {
            self.unpack_zip(archive, dest)?;
        } else {
            return Err(CoreError::ExtractionFailed(format!(
                "Unknown archive format: {}",
                filename
            )));
        }

        Ok(dest.to_path_buf())
    }

    /// Unpack a .tar.gz archive.
    fn unpack_tar_gz(&self, archive: &Path, dest: &Path) -> Result<()> {
        let file = File::open(archive)?;
        let decoder = GzDecoder::new(BufReader::new(file));
        let mut archive = Archive::new(decoder);
        archive.unpack(dest)?;
        Ok(())
    }

    /// Unpack a .tar archive.
    fn unpack_tar(&self, archive: &Path, dest: &Path) -> Result<()> {
        let file = File::open(archive)?;
        let mut archive = Archive::new(BufReader::new(file));
        archive.unpack(dest)?;
        Ok(())
    }

    /// Unpack a .zip archive.
    fn unpack_zip(&self, archive: &Path, dest: &Path) -> Result<()> {
        let file = File::open(archive)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| CoreError::ExtractionFailed(format!("Failed to open zip: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                CoreError::ExtractionFailed(format!("Failed to read zip entry: {}", e))
            })?;

            let outpath = match file.enclosed_name() {
                Some(path) => dest.join(path),
                None => continue,
            };

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }

            // Set permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }
        }

        Ok(())
    }

    // ========== Filesystem Operations ==========

    /// Copy a file or directory.
    pub fn copy(&self, src: &Path, dst: &Path) -> Result<()> {
        trace!("Copying {} to {}", src.display(), dst.display());

        if src.is_dir() {
            self.copy_dir_recursive(src, dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(src, dst)?;
        }

        Ok(())
    }

    /// Copy a directory recursively.
    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;

        for entry in walkdir::WalkDir::new(src) {
            let entry = entry.map_err(|e| CoreError::FileOperation {
                path: src.display().to_string(),
                message: e.to_string(),
            })?;

            let rel_path = entry.path().strip_prefix(src).unwrap_or(entry.path());
            let dst_path = dst.join(rel_path);

            if entry.file_type().is_dir() {
                fs::create_dir_all(&dst_path)?;
            } else {
                if let Some(parent) = dst_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(entry.path(), &dst_path)?;
            }
        }

        Ok(())
    }

    /// Move a file or directory.
    pub fn move_path(&self, src: &Path, dst: &Path) -> Result<()> {
        trace!("Moving {} to {}", src.display(), dst.display());

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        // Try rename first (fast path for same filesystem)
        if fs::rename(src, dst).is_ok() {
            return Ok(());
        }

        // Fall back to copy + remove
        self.copy(src, dst)?;
        if src.is_dir() {
            fs::remove_dir_all(src)?;
        } else {
            fs::remove_file(src)?;
        }

        Ok(())
    }

    /// Create a directory (recursive).
    pub fn mkdir(&self, path: &Path) -> Result<()> {
        trace!("Creating directory: {}", path.display());
        fs::create_dir_all(path)?;
        Ok(())
    }

    /// Write content to a file.
    pub fn write(&self, path: &Path, content: &str) -> Result<()> {
        trace!("Writing to {}", path.display());

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;

        Ok(())
    }

    /// Set file permissions (Unix).
    #[cfg(unix)]
    pub fn chmod(&self, path: &Path, mode: u32) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        trace!("Setting permissions on {} to {:o}", path.display(), mode);
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        Ok(())
    }

    /// Set file permissions (Windows - no-op).
    #[cfg(windows)]
    pub fn chmod(&self, _path: &Path, _mode: u32) -> Result<()> {
        // Windows doesn't have Unix-style permissions
        Ok(())
    }

    /// Create a symbolic link.
    pub fn symlink(&self, target: &Path, link: &Path) -> Result<()> {
        trace!(
            "Creating symlink {} -> {}",
            link.display(),
            target.display()
        );

        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }

        // Remove existing symlink if present
        if link.is_symlink() {
            fs::remove_file(link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link)?;

        #[cfg(windows)]
        {
            if target.is_dir() {
                std::os::windows::fs::symlink_dir(target, link)?;
            } else {
                std::os::windows::fs::symlink_file(target, link)?;
            }
        }

        Ok(())
    }

    // ========== Shell Execution ==========

    /// Run a shell command.
    ///
    /// Options:
    /// - `cwd`: Working directory (default: temp_dir)
    /// - `shell`: Shell to use (default: sh on Unix, powershell on Windows)
    pub fn run(&self, cmd: &str, cwd: Option<&Path>) -> Result<String> {
        let cwd = cwd.unwrap_or(&self.temp_dir);
        debug!("Running command in {}: {}", cwd.display(), cmd);

        #[cfg(unix)]
        let (shell, args) = ("sh", vec!["-c", cmd]);

        #[cfg(windows)]
        let (shell, args) = ("powershell", vec!["-Command", cmd]);

        let mut command = Command::new(shell);
        command.args(&args).current_dir(cwd);

        // Add environment variables
        for (key, value) in &self.env {
            command.env(key, value);
        }

        let output = command.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::BuildFailed {
                name: "command".to_string(),
                message: format!("Command failed with status {}: {}", output.status, stderr),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_context() -> (BuildContext, TempDir) {
        let temp = TempDir::new().unwrap();
        let out_dir = temp.path().join("out");
        let build_temp = temp.path().join("tmp");
        fs::create_dir_all(&out_dir).unwrap();
        fs::create_dir_all(&build_temp).unwrap();

        (BuildContext::new(out_dir, build_temp), temp)
    }

    #[test]
    fn test_mkdir() {
        let (ctx, temp) = setup_context();
        let dir = temp.path().join("nested/dir/path");

        ctx.mkdir(&dir).unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());
    }

    #[test]
    fn test_write() {
        let (ctx, temp) = setup_context();
        let file = temp.path().join("nested/test.txt");

        ctx.write(&file, "hello world").unwrap();
        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello world");
    }

    #[test]
    fn test_copy_file() {
        let (ctx, temp) = setup_context();

        // Create source file
        let src = temp.path().join("src.txt");
        fs::write(&src, "content").unwrap();

        // Copy it
        let dst = temp.path().join("dst.txt");
        ctx.copy(&src, &dst).unwrap();

        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
    }

    #[test]
    fn test_copy_dir() {
        let (ctx, temp) = setup_context();

        // Create source directory with files
        let src = temp.path().join("src_dir");
        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("file.txt"), "a").unwrap();
        fs::write(src.join("subdir/nested.txt"), "b").unwrap();

        // Copy it
        let dst = temp.path().join("dst_dir");
        ctx.copy(&src, &dst).unwrap();

        assert!(dst.join("file.txt").exists());
        assert!(dst.join("subdir/nested.txt").exists());
        assert_eq!(
            fs::read_to_string(dst.join("subdir/nested.txt")).unwrap(),
            "b"
        );
    }

    #[test]
    fn test_move_path() {
        let (ctx, temp) = setup_context();

        let src = temp.path().join("src.txt");
        fs::write(&src, "content").unwrap();

        let dst = temp.path().join("dst.txt");
        ctx.move_path(&src, &dst).unwrap();

        assert!(!src.exists());
        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
    }

    #[test]
    fn test_symlink() {
        let (ctx, temp) = setup_context();

        let target = temp.path().join("target.txt");
        fs::write(&target, "content").unwrap();

        let link = temp.path().join("link.txt");
        ctx.symlink(&target, &link).unwrap();

        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(&link).unwrap(), "content");
    }

    #[test]
    #[cfg(unix)]
    fn test_chmod() {
        use std::os::unix::fs::PermissionsExt;

        let (ctx, temp) = setup_context();
        let file = temp.path().join("test.txt");
        fs::write(&file, "").unwrap();

        ctx.chmod(&file, 0o755).unwrap();

        let perms = fs::metadata(&file).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o755);
    }

    #[test]
    fn test_run() {
        let (ctx, _temp) = setup_context();

        let output = ctx.run("echo hello", None).unwrap();
        assert!(output.trim().contains("hello"));
    }

    #[test]
    fn test_unpack_tar_gz() {
        let (ctx, temp) = setup_context();

        // Create a simple tar.gz archive
        let archive_path = temp.path().join("test.tar.gz");

        // Create some content to archive
        let content_dir = temp.path().join("content");
        fs::create_dir_all(&content_dir).unwrap();
        fs::write(content_dir.join("file.txt"), "hello").unwrap();

        // Use tar command to create archive
        Command::new("tar")
            .args(["czf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(temp.path())
            .arg("content")
            .status()
            .unwrap();

        // Unpack it
        let unpack_dir = temp.path().join("unpacked");
        ctx.unpack(&archive_path, Some(&unpack_dir)).unwrap();

        assert!(unpack_dir.join("content/file.txt").exists());
    }
}
