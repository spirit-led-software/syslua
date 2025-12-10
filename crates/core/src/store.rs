//! Store module for sys.lua
//!
//! The store is the realization engine for derivations. Every object in `store/obj/`
//! is the output of realizing a derivation. Objects use a human-readable naming scheme:
//! `obj/name-version-hash/` (or `obj/name-hash/` if no version).

use crate::Result;
use crate::derivation::{Derivation, DerivationSpec};
use crate::error::CoreError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, trace};

/// Length of truncated hash for store paths (9 characters for readability)
const HASH_TRUNCATE_LEN: usize = 9;

/// The store manages derivation outputs and provides content-addressed storage.
#[derive(Debug, Clone)]
pub struct Store {
    /// Root path of the store (e.g., `~/.local/share/syslua/store/`)
    root: PathBuf,
}

impl Store {
    /// Create a new store at the given root path.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Create a store at the default user location.
    ///
    /// - Linux: `~/.local/share/syslua/store`
    /// - macOS: `~/Library/Application Support/syslua/store`
    /// - Windows: `%LOCALAPPDATA%\syslua\store`
    pub fn user_store() -> Option<Self> {
        dirs::data_local_dir().map(|d| Self::new(d.join("syslua").join("store")))
    }

    /// Create a store at the system location (requires elevated privileges).
    ///
    /// - Linux/macOS: `/syslua/store`
    /// - Windows: `C:\syslua\store`
    pub fn system_store() -> Self {
        #[cfg(unix)]
        let root = PathBuf::from("/syslua/store");
        #[cfg(windows)]
        let root = PathBuf::from("C:\\syslua\\store");
        Self::new(root)
    }

    /// Get the store root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the path to the objects directory (`store/obj/`).
    pub fn obj_dir(&self) -> PathBuf {
        self.root.join("obj")
    }

    /// Get the path to the derivations directory (`store/drv/`).
    pub fn drv_dir(&self) -> PathBuf {
        self.root.join("drv")
    }

    /// Get the path to the derivation-to-output mapping directory (`store/drv-out/`).
    pub fn drv_out_dir(&self) -> PathBuf {
        self.root.join("drv-out")
    }

    /// Get the path to the package symlinks directory (`store/pkg/`).
    pub fn pkg_dir(&self) -> PathBuf {
        self.root.join("pkg")
    }

    /// Get the path to the metadata directory (`store/metadata/`).
    pub fn metadata_dir(&self) -> PathBuf {
        self.root.join("metadata")
    }

    /// Compute the store object path for a derivation.
    ///
    /// Format: `obj/<name>-<version>-<hash>/` or `obj/<name>-<hash>/` if no version.
    pub fn object_path(&self, name: &str, version: Option<&str>, hash: &str) -> PathBuf {
        let truncated_hash = truncate_hash(hash);
        let dir_name = match version {
            Some(v) => format!("{}-{}-{}", name, v, truncated_hash),
            None => format!("{}-{}", name, truncated_hash),
        };
        self.obj_dir().join(dir_name)
    }

    /// Compute the package link path.
    ///
    /// Format: `pkg/<name>/<version>/<platform>` pointing to the object.
    pub fn package_link_path(&self, name: &str, version: &str, platform: &str) -> PathBuf {
        self.pkg_dir().join(name).join(version).join(platform)
    }

    /// Get the path to a derivation spec file.
    ///
    /// Format: `drv/<hash>.drv`
    pub fn derivation_path(&self, hash: &str) -> PathBuf {
        self.drv_dir().join(format!("{}.drv", hash))
    }

    /// Get the path to a derivation-to-output mapping.
    ///
    /// Format: `drv-out/<drv_hash>` (contents are the output hash)
    pub fn drv_out_path(&self, drv_hash: &str) -> PathBuf {
        self.drv_out_dir().join(drv_hash)
    }

    /// Initialize the store directory structure.
    pub fn init(&self) -> Result<()> {
        info!("Initializing store at {}", self.root.display());

        fs::create_dir_all(self.obj_dir())?;
        fs::create_dir_all(self.drv_dir())?;
        fs::create_dir_all(self.drv_out_dir())?;
        fs::create_dir_all(self.pkg_dir())?;
        fs::create_dir_all(self.metadata_dir())?;

        debug!("Store directories created");
        Ok(())
    }

    /// Check if an object exists in the store.
    pub fn has_object(&self, name: &str, version: Option<&str>, hash: &str) -> bool {
        self.object_path(name, version, hash).exists()
    }

    /// Look up a cached output hash for a derivation hash.
    ///
    /// Returns `Some(output_hash)` if the derivation has been built before.
    pub fn lookup_cache(&self, drv_hash: &str) -> Option<String> {
        let cache_path = self.drv_out_path(drv_hash);
        if cache_path.exists() {
            fs::read_to_string(&cache_path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    }

    /// Cache the mapping from derivation hash to output hash.
    pub fn cache_output(&self, drv_hash: &str, output_hash: &str) -> Result<()> {
        let cache_path = self.drv_out_path(drv_hash);
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&cache_path, output_hash)?;
        trace!("Cached drv {} -> output {}", drv_hash, output_hash);
        Ok(())
    }

    /// Save a derivation spec to the store.
    pub fn save_derivation(&self, drv: &Derivation) -> Result<()> {
        let drv_path = self.derivation_path(&drv.hash);
        if let Some(parent) = drv_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&drv.spec)?;
        fs::write(&drv_path, json)?;
        trace!("Saved derivation {} to {}", drv.hash, drv_path.display());

        Ok(())
    }

    /// Load a derivation spec from the store.
    pub fn load_derivation(&self, hash: &str) -> Result<DerivationSpec> {
        let drv_path = self.derivation_path(hash);
        if !drv_path.exists() {
            return Err(CoreError::ObjectNotFound(format!(
                "Derivation {} not found",
                hash
            )));
        }

        let json = fs::read_to_string(&drv_path)?;
        let spec: DerivationSpec = serde_json::from_str(&json)?;
        Ok(spec)
    }

    /// Finalize a build output by moving it to the store and making it immutable.
    ///
    /// This:
    /// 1. Computes the content hash of the output directory
    /// 2. Moves it to the final store location
    /// 3. Makes it immutable
    /// 4. Caches the derivation -> output mapping
    pub fn finalize_output(&self, drv: &Derivation, build_output: &Path) -> Result<PathBuf> {
        // Compute content hash of the output
        let output_hash = sha256_directory(build_output)?;
        debug!(
            "Output hash for {}: {}",
            drv.name(),
            truncate_hash(&output_hash)
        );

        // Determine final store path
        let final_path = self.object_path(drv.name(), drv.version(), &output_hash);

        // If already exists (same content), we're done
        if final_path.exists() {
            info!("Store object already exists: {}", final_path.display());
            // Still cache the mapping
            self.cache_output(&drv.hash, &output_hash)?;
            return Ok(final_path);
        }

        // Create parent directory
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Move to final location (atomic on same filesystem)
        if fs::rename(build_output, &final_path).is_err() {
            // Fall back to copy + remove
            copy_dir_all(build_output, &final_path)?;
            fs::remove_dir_all(build_output)?;
        }

        // Make immutable
        self.make_immutable(&final_path)?;

        // Cache the mapping
        self.cache_output(&drv.hash, &output_hash)?;

        info!("Stored {} at {}", drv.name(), final_path.display());

        Ok(final_path)
    }

    /// Make a directory and its contents immutable.
    #[cfg(unix)]
    fn make_immutable(&self, path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        for entry in walkdir::WalkDir::new(path) {
            let entry = entry.map_err(|e| CoreError::FileOperation {
                path: path.display().to_string(),
                message: e.to_string(),
            })?;

            let metadata = entry.metadata().map_err(|e| CoreError::FileOperation {
                path: entry.path().display().to_string(),
                message: e.to_string(),
            })?;

            // Make files read-only
            let mut perms = metadata.permissions();
            let mode = perms.mode();
            // Remove write bits
            perms.set_mode(mode & !0o222);
            fs::set_permissions(entry.path(), perms)?;
        }

        // Note: For full immutability we'd use chattr +i (Linux) or chflags uchg (macOS)
        // but that requires elevated privileges. Read-only is good enough for user stores.

        Ok(())
    }

    /// Make a directory and its contents immutable (Windows version).
    #[cfg(windows)]
    fn make_immutable(&self, path: &Path) -> Result<()> {
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry.map_err(|e| CoreError::FileOperation {
                path: path.display().to_string(),
                message: e.to_string(),
            })?;

            let metadata = entry.metadata().map_err(|e| CoreError::FileOperation {
                path: entry.path().display().to_string(),
                message: e.to_string(),
            })?;

            // Make files read-only
            let mut perms = metadata.permissions();
            perms.set_readonly(true);
            fs::set_permissions(entry.path(), perms)?;
        }

        Ok(())
    }

    /// Create a package symlink pointing to a store object.
    pub fn create_package_link(
        &self,
        name: &str,
        version: &str,
        platform: &str,
        object_path: &Path,
    ) -> Result<PathBuf> {
        let link_path = self.package_link_path(name, version, platform);

        if let Some(parent) = link_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Remove existing link if present
        if link_path.is_symlink() || link_path.exists() {
            if link_path.is_symlink() {
                fs::remove_file(&link_path)?;
            } else {
                fs::remove_dir_all(&link_path)?;
            }
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(object_path, &link_path)?;

        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(object_path, &link_path)?;

        debug!(
            "Created package link {} -> {}",
            link_path.display(),
            object_path.display()
        );

        Ok(link_path)
    }
}

/// Truncate a hash to the display length for store paths.
pub fn truncate_hash(hash: &str) -> &str {
    if hash.len() > HASH_TRUNCATE_LEN {
        &hash[..HASH_TRUNCATE_LEN]
    } else {
        hash
    }
}

/// Compute a SHA-256 hash of the given bytes, returning the full hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Compute a SHA-256 hash of a string, returning the full hex string.
pub fn sha256_string(s: &str) -> String {
    sha256_hex(s.as_bytes())
}

/// Compute a SHA-256 hash of a file, returning the full hex string.
pub fn sha256_file(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    Ok(sha256_hex(&data))
}

/// Compute a SHA-256 hash of a directory's contents.
///
/// This walks all files in sorted order and hashes their paths and contents.
pub fn sha256_directory(path: &Path) -> Result<String> {
    use walkdir::WalkDir;

    let mut hasher = Sha256::new();

    // Collect and sort entries for deterministic hashing
    let mut entries: Vec<_> = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    entries.sort_by(|a, b| a.path().cmp(b.path()));

    for entry in entries {
        // Hash the relative path
        let rel_path = entry
            .path()
            .strip_prefix(path)
            .unwrap_or(entry.path())
            .to_string_lossy();
        hasher.update(rel_path.as_bytes());
        hasher.update(b"\0");

        // Hash the file contents
        let contents = fs::read(entry.path())?;
        hasher.update(&contents);
        hasher.update(b"\0");
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Copy a directory recursively.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    use walkdir::WalkDir;

    fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_truncate_hash() {
        let hash = "abc123def456789";
        assert_eq!(truncate_hash(hash), "abc123def");

        let short = "abc";
        assert_eq!(truncate_hash(short), "abc");
    }

    #[test]
    fn test_sha256_string() {
        // Known SHA-256 hash of "hello"
        let hash = sha256_string("hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_store_paths() {
        let store = Store::new("/syslua/store");

        // Object path with version
        let obj = store.object_path("ripgrep", Some("15.1.0"), "abc123def456789");
        assert_eq!(
            obj.to_str().unwrap(),
            "/syslua/store/obj/ripgrep-15.1.0-abc123def"
        );

        // Object path without version
        let obj = store.object_path("my-config", None, "abc123def456789");
        assert_eq!(
            obj.to_str().unwrap(),
            "/syslua/store/obj/my-config-abc123def"
        );

        // Package link path
        let pkg = store.package_link_path("ripgrep", "15.1.0", "aarch64-darwin");
        assert_eq!(
            pkg.to_str().unwrap(),
            "/syslua/store/pkg/ripgrep/15.1.0/aarch64-darwin"
        );

        // Derivation path
        let drv = store.derivation_path("abc123");
        assert_eq!(drv.to_str().unwrap(), "/syslua/store/drv/abc123.drv");
    }

    #[test]
    fn test_store_init() {
        let temp = TempDir::new().unwrap();
        let store = Store::new(temp.path().join("store"));

        store.init().unwrap();

        assert!(store.obj_dir().exists());
        assert!(store.drv_dir().exists());
        assert!(store.drv_out_dir().exists());
        assert!(store.pkg_dir().exists());
        assert!(store.metadata_dir().exists());
    }

    #[test]
    fn test_cache_operations() {
        let temp = TempDir::new().unwrap();
        let store = Store::new(temp.path().join("store"));
        store.init().unwrap();

        let drv_hash = "abc123";
        let output_hash = "def456";

        // Initially no cache entry
        assert!(store.lookup_cache(drv_hash).is_none());

        // Cache the output
        store.cache_output(drv_hash, output_hash).unwrap();

        // Now should find it
        assert_eq!(store.lookup_cache(drv_hash), Some(output_hash.to_string()));
    }

    #[test]
    fn test_sha256_directory() {
        let temp = TempDir::new().unwrap();

        // Create some files
        fs::write(temp.path().join("a.txt"), "hello").unwrap();
        fs::create_dir(temp.path().join("subdir")).unwrap();
        fs::write(temp.path().join("subdir/b.txt"), "world").unwrap();

        let hash = sha256_directory(temp.path()).unwrap();

        // Hash should be consistent
        let hash2 = sha256_directory(temp.path()).unwrap();
        assert_eq!(hash, hash2);

        // Modifying a file should change the hash
        fs::write(temp.path().join("a.txt"), "goodbye").unwrap();
        let hash3 = sha256_directory(temp.path()).unwrap();
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_save_and_load_derivation() {
        use crate::derivation::{DerivationSpec, System};
        use std::collections::BTreeMap;

        let temp = TempDir::new().unwrap();
        let store = Store::new(temp.path().join("store"));
        store.init().unwrap();

        let spec = DerivationSpec {
            name: "test-pkg".to_string(),
            version: Some("1.0.0".to_string()),
            inputs: BTreeMap::new(),
            build_hash: "buildhash123".to_string(),
            outputs: vec!["out".to_string()],
            system: System {
                platform: "x86_64-linux".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                hostname: "test".to_string(),
                username: "user".to_string(),
            },
        };

        let drv = Derivation::new(spec.clone());

        // Save derivation
        store.save_derivation(&drv).unwrap();

        // Load it back
        let loaded = store.load_derivation(&drv.hash).unwrap();
        assert_eq!(loaded.name, spec.name);
        assert_eq!(loaded.version, spec.version);
    }

    #[test]
    fn test_finalize_output() {
        use crate::derivation::{DerivationSpec, System};
        use std::collections::BTreeMap;

        let temp = TempDir::new().unwrap();
        let store = Store::new(temp.path().join("store"));
        store.init().unwrap();

        // Create a derivation
        let spec = DerivationSpec {
            name: "test-pkg".to_string(),
            version: Some("1.0.0".to_string()),
            inputs: BTreeMap::new(),
            build_hash: "buildhash456".to_string(),
            outputs: vec!["out".to_string()],
            system: System {
                platform: "x86_64-linux".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                hostname: "test".to_string(),
                username: "user".to_string(),
            },
        };
        let drv = Derivation::new(spec);

        // Create build output
        let build_out = temp.path().join("build_out");
        fs::create_dir_all(build_out.join("bin")).unwrap();
        fs::write(build_out.join("bin/test"), "#!/bin/sh\necho hello").unwrap();

        // Finalize it
        let store_path = store.finalize_output(&drv, &build_out).unwrap();

        // Verify
        assert!(store_path.exists());
        assert!(store_path.join("bin/test").exists());

        // Cache should be populated
        assert!(store.lookup_cache(&drv.hash).is_some());
    }

    #[test]
    fn test_create_package_link() {
        let temp = TempDir::new().unwrap();
        let store = Store::new(temp.path().join("store"));
        store.init().unwrap();

        // Create a fake object directory
        let obj_path = store.obj_dir().join("test-pkg-1.0.0-abc123");
        fs::create_dir_all(&obj_path).unwrap();
        fs::write(obj_path.join("bin"), "content").unwrap();

        // Create package link
        let link = store
            .create_package_link("test-pkg", "1.0.0", "x86_64-linux", &obj_path)
            .unwrap();

        assert!(link.is_symlink());
        assert_eq!(fs::read_link(&link).unwrap(), obj_path);
    }
}
