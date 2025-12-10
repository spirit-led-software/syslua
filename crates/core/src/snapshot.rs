//! Snapshot module for sys.lua
//!
//! Snapshots capture the complete state of the system at a point in time,
//! enabling rollback to previous configurations.

use crate::Result;
use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// A snapshot of the system state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier (timestamp-based)
    pub id: String,
    /// Unix timestamp when the snapshot was created
    pub created_at: u64,
    /// Human-readable description
    pub description: String,
    /// Path to the configuration file that was applied
    pub config_path: Option<PathBuf>,
    /// Content of the configuration file (for reproducibility)
    pub config_content: Option<String>,
    /// Files managed by this snapshot
    pub files: Vec<SnapshotFile>,
    /// Environment variables
    pub envs: Vec<SnapshotEnv>,
    /// Derivations that were built
    pub derivations: Vec<SnapshotDerivation>,
}

impl Snapshot {
    /// Create a new snapshot with a generated ID.
    pub fn new(description: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            id: now.to_string(),
            created_at: now / 1000, // Convert to seconds for created_at
            description: description.into(),
            config_path: None,
            config_content: None,
            files: Vec::new(),
            envs: Vec::new(),
            derivations: Vec::new(),
        }
    }

    /// Set the configuration file information.
    pub fn with_config(mut self, path: &Path) -> Self {
        self.config_path = Some(path.to_path_buf());
        if let Ok(content) = fs::read_to_string(path) {
            self.config_content = Some(content);
        }
        self
    }

    /// Add a file to the snapshot.
    pub fn add_file(&mut self, file: SnapshotFile) {
        self.files.push(file);
    }

    /// Add an environment variable to the snapshot.
    pub fn add_env(&mut self, env: SnapshotEnv) {
        self.envs.push(env);
    }

    /// Add a derivation to the snapshot.
    pub fn add_derivation(&mut self, drv: SnapshotDerivation) {
        self.derivations.push(drv);
    }

    /// Get the creation time as a formatted string.
    pub fn created_at_formatted(&self) -> String {
        use std::time::{Duration, UNIX_EPOCH};
        let datetime = UNIX_EPOCH + Duration::from_secs(self.created_at);
        // Simple ISO-like format (without external crates)
        format!("{:?}", datetime)
    }
}

/// A file captured in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// Target path where the file is linked/placed
    pub path: PathBuf,
    /// Type of file management
    pub file_type: SnapshotFileType,
    /// Content hash (if store-backed)
    pub hash: Option<String>,
    /// File permissions mode
    pub mode: Option<u32>,
    /// Symlink target (for symlinks)
    pub target: Option<PathBuf>,
    /// Derivation hash that produced this file
    pub derivation_hash: Option<String>,
}

impl SnapshotFile {
    /// Create a new snapshot file entry for a store-backed file.
    pub fn store_backed(path: PathBuf, hash: String, derivation_hash: String) -> Self {
        Self {
            path,
            file_type: SnapshotFileType::StoreBacked,
            hash: Some(hash),
            mode: None,
            target: None,
            derivation_hash: Some(derivation_hash),
        }
    }

    /// Create a new snapshot file entry for a mutable symlink.
    pub fn mutable_symlink(path: PathBuf, target: PathBuf) -> Self {
        Self {
            path,
            file_type: SnapshotFileType::MutableSymlink,
            hash: None,
            mode: None,
            target: Some(target),
            derivation_hash: None,
        }
    }

    /// Create a new snapshot file entry from current filesystem state.
    pub fn from_path(path: &Path) -> Option<Self> {
        if !path.exists() && !path.is_symlink() {
            return None;
        }

        let metadata = path.symlink_metadata().ok()?;

        if metadata.file_type().is_symlink() {
            let target = fs::read_link(path).ok()?;
            Some(Self {
                path: path.to_path_buf(),
                file_type: SnapshotFileType::MutableSymlink,
                hash: None,
                mode: None,
                target: Some(target),
                derivation_hash: None,
            })
        } else {
            // Regular file - would need content backup for rollback
            Some(Self {
                path: path.to_path_buf(),
                file_type: SnapshotFileType::RegularFile,
                hash: None,
                mode: Some(metadata.permissions().readonly() as u32),
                target: None,
                derivation_hash: None,
            })
        }
    }
}

/// Type of file in a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotFileType {
    /// Content stored in store, symlinked to target
    StoreBacked,
    /// Direct symlink to source (mutable mode)
    MutableSymlink,
    /// Regular file (backed up content)
    RegularFile,
}

/// An environment variable captured in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEnv {
    /// Variable name
    pub name: String,
    /// Variable value
    pub value: String,
    /// Merge strategy used
    pub merge_strategy: String,
    /// Derivation hash that produced this env
    pub derivation_hash: Option<String>,
}

impl SnapshotEnv {
    /// Create a new snapshot env entry.
    pub fn new(name: String, value: String, merge_strategy: &str) -> Self {
        Self {
            name,
            value,
            merge_strategy: merge_strategy.to_string(),
            derivation_hash: None,
        }
    }

    /// Set the derivation hash.
    pub fn with_derivation(mut self, hash: String) -> Self {
        self.derivation_hash = Some(hash);
        self
    }
}

/// A derivation captured in a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDerivation {
    /// Derivation name
    pub name: String,
    /// Derivation version
    pub version: Option<String>,
    /// Derivation hash
    pub hash: String,
    /// Output path in store
    pub output_path: Option<PathBuf>,
    /// Type of derivation (file, env, package)
    pub derivation_type: String,
}

impl SnapshotDerivation {
    /// Create a new snapshot derivation entry.
    pub fn new(name: String, version: Option<String>, hash: String, derivation_type: &str) -> Self {
        Self {
            name,
            version,
            hash,
            output_path: None,
            derivation_type: derivation_type.to_string(),
        }
    }

    /// Set the output path.
    pub fn with_output(mut self, path: PathBuf) -> Self {
        self.output_path = Some(path);
        self
    }
}

/// Metadata file containing snapshot index and current pointer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SnapshotMetadata {
    /// Version of the metadata format
    pub version: u32,
    /// List of all snapshots (ordered by creation time)
    pub snapshots: Vec<SnapshotSummary>,
    /// ID of the current/latest snapshot
    pub current: Option<String>,
}

/// Summary information about a snapshot (stored in metadata index).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    /// Snapshot ID
    pub id: String,
    /// Unix timestamp
    pub created_at: u64,
    /// Description
    pub description: String,
    /// Number of files
    pub file_count: usize,
    /// Number of derivations
    pub derivation_count: usize,
}

impl From<&Snapshot> for SnapshotSummary {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            created_at: snapshot.created_at,
            description: snapshot.description.clone(),
            file_count: snapshot.files.len(),
            derivation_count: snapshot.derivations.len(),
        }
    }
}

/// Manages snapshots storage and retrieval.
pub struct SnapshotManager {
    /// Base directory for snapshots
    snapshots_dir: PathBuf,
    /// Path to metadata file
    metadata_path: PathBuf,
    /// Directory for backed up file contents
    files_dir: PathBuf,
}

impl SnapshotManager {
    /// Create a new snapshot manager.
    pub fn new(snapshots_dir: impl Into<PathBuf>) -> Self {
        let snapshots_dir = snapshots_dir.into();
        Self {
            metadata_path: snapshots_dir.join("metadata.json"),
            files_dir: snapshots_dir.join("files"),
            snapshots_dir,
        }
    }

    /// Initialize the snapshot storage directory structure.
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.snapshots_dir)?;
        fs::create_dir_all(&self.files_dir)?;

        // Create metadata file if it doesn't exist
        if !self.metadata_path.exists() {
            let metadata = SnapshotMetadata {
                version: 1,
                snapshots: Vec::new(),
                current: None,
            };
            self.save_metadata(&metadata)?;
        }

        Ok(())
    }

    /// Get the path where a specific snapshot is stored.
    fn snapshot_path(&self, id: &str) -> PathBuf {
        self.snapshots_dir.join(format!("{}.json", id))
    }

    /// Get the directory for backing up files for a snapshot.
    fn snapshot_files_dir(&self, id: &str) -> PathBuf {
        self.files_dir.join(id)
    }

    /// Load the metadata index.
    pub fn load_metadata(&self) -> Result<SnapshotMetadata> {
        if !self.metadata_path.exists() {
            return Ok(SnapshotMetadata::default());
        }

        let content = fs::read_to_string(&self.metadata_path)?;
        let metadata: SnapshotMetadata = serde_json::from_str(&content)?;
        Ok(metadata)
    }

    /// Save the metadata index.
    fn save_metadata(&self, metadata: &SnapshotMetadata) -> Result<()> {
        let content = serde_json::to_string_pretty(metadata)?;
        fs::write(&self.metadata_path, content)?;
        Ok(())
    }

    /// Create and save a new snapshot.
    pub fn create_snapshot(&self, snapshot: Snapshot) -> Result<String> {
        self.init()?;

        let id = snapshot.id.clone();
        debug!("Creating snapshot {}: {}", id, snapshot.description);

        // Save the full snapshot
        let snapshot_path = self.snapshot_path(&id);
        let content = serde_json::to_string_pretty(&snapshot)?;
        fs::write(&snapshot_path, content)?;

        // Update metadata
        let mut metadata = self.load_metadata()?;
        metadata.snapshots.push(SnapshotSummary::from(&snapshot));
        metadata.current = Some(id.clone());
        self.save_metadata(&metadata)?;

        info!("Created snapshot {}", id);
        Ok(id)
    }

    /// Get a snapshot by ID.
    pub fn get_snapshot(&self, id: &str) -> Result<Snapshot> {
        let snapshot_path = self.snapshot_path(id);
        if !snapshot_path.exists() {
            return Err(CoreError::SnapshotNotFound(id.to_string()));
        }

        let content = fs::read_to_string(&snapshot_path)?;
        let snapshot: Snapshot = serde_json::from_str(&content)?;
        Ok(snapshot)
    }

    /// Get the current/latest snapshot.
    pub fn get_current_snapshot(&self) -> Result<Option<Snapshot>> {
        let metadata = self.load_metadata()?;
        match metadata.current {
            Some(id) => Ok(Some(self.get_snapshot(&id)?)),
            None => Ok(None),
        }
    }

    /// List all snapshots (summaries).
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotSummary>> {
        let metadata = self.load_metadata()?;
        Ok(metadata.snapshots)
    }

    /// Get the ID of the current snapshot.
    pub fn get_current_id(&self) -> Result<Option<String>> {
        let metadata = self.load_metadata()?;
        Ok(metadata.current)
    }

    /// Delete a snapshot.
    pub fn delete_snapshot(&self, id: &str) -> Result<()> {
        let snapshot_path = self.snapshot_path(id);
        if !snapshot_path.exists() {
            return Err(CoreError::SnapshotNotFound(id.to_string()));
        }

        // Remove snapshot file
        fs::remove_file(&snapshot_path)?;

        // Remove backed up files if any
        let files_dir = self.snapshot_files_dir(id);
        if files_dir.exists() {
            fs::remove_dir_all(&files_dir)?;
        }

        // Update metadata
        let mut metadata = self.load_metadata()?;
        metadata.snapshots.retain(|s| s.id != id);
        if metadata.current.as_deref() == Some(id) {
            // Set current to the previous snapshot if available
            metadata.current = metadata.snapshots.last().map(|s| s.id.clone());
        }
        self.save_metadata(&metadata)?;

        info!("Deleted snapshot {}", id);
        Ok(())
    }

    /// Backup a file's content for rollback (for non-store-backed files).
    pub fn backup_file(&self, snapshot_id: &str, path: &Path) -> Result<Option<PathBuf>> {
        if !path.exists() {
            return Ok(None);
        }

        let backup_dir = self.snapshot_files_dir(snapshot_id);
        fs::create_dir_all(&backup_dir)?;

        // Create a safe filename from the path
        let safe_name = path
            .to_string_lossy()
            .replace(['/', '\\', ':'], "_");
        let backup_path = backup_dir.join(&safe_name);

        // Copy the file
        fs::copy(path, &backup_path)?;
        debug!("Backed up {} to {}", path.display(), backup_path.display());

        Ok(Some(backup_path))
    }

    /// Get the backed up file path for a snapshot.
    pub fn get_backup_path(&self, snapshot_id: &str, original_path: &Path) -> PathBuf {
        let safe_name = original_path
            .to_string_lossy()
            .replace(['/', '\\', ':'], "_");
        self.snapshot_files_dir(snapshot_id).join(safe_name)
    }

    /// Perform a rollback to a specific snapshot.
    ///
    /// This restores the system state to match the snapshot:
    /// - Removes files not in the snapshot
    /// - Restores files from the snapshot
    /// - Re-creates symlinks
    pub fn rollback_to(&self, target_id: &str) -> Result<RollbackResult> {
        info!("Rolling back to snapshot {}", target_id);

        let target = self.get_snapshot(target_id)?;
        let current = self.get_current_snapshot()?;

        let mut result = RollbackResult {
            target_id: target_id.to_string(),
            files_restored: Vec::new(),
            files_removed: Vec::new(),
            errors: Vec::new(),
        };

        // Get current files to compare
        let current_files: BTreeMap<PathBuf, &SnapshotFile> = current
            .as_ref()
            .map(|s| s.files.iter().map(|f| (f.path.clone(), f)).collect())
            .unwrap_or_default();

        let target_files: BTreeMap<PathBuf, &SnapshotFile> =
            target.files.iter().map(|f| (f.path.clone(), f)).collect();

        // Remove files that are in current but not in target
        for path in current_files.keys() {
            if !target_files.contains_key(path) {
                if let Err(e) = self.remove_managed_file(path) {
                    result
                        .errors
                        .push(format!("Failed to remove {}: {}", path.display(), e));
                } else {
                    result.files_removed.push(path.clone());
                }
            }
        }

        // Restore files from target snapshot
        for file in target_files.values() {
            match self.restore_file(&target, file) {
                Ok(true) => result.files_restored.push(file.path.clone()),
                Ok(false) => {} // No change needed
                Err(e) => {
                    result
                        .errors
                        .push(format!("Failed to restore {}: {}", file.path.display(), e))
                }
            }
        }

        // Update current pointer
        let mut metadata = self.load_metadata()?;
        metadata.current = Some(target_id.to_string());
        self.save_metadata(&metadata)?;

        if result.errors.is_empty() {
            info!("Rollback completed successfully");
        } else {
            warn!("Rollback completed with {} errors", result.errors.len());
        }

        Ok(result)
    }

    /// Remove a file managed by sys.lua.
    fn remove_managed_file(&self, path: &Path) -> Result<()> {
        if path.is_symlink() || path.exists() {
            fs::remove_file(path)?;
        }
        debug!("Removed managed file: {}", path.display());
        Ok(())
    }

    /// Restore a file from a snapshot.
    ///
    /// Returns Ok(true) if the file was restored, Ok(false) if no change was needed.
    fn restore_file(&self, snapshot: &Snapshot, file: &SnapshotFile) -> Result<bool> {
        match file.file_type {
            SnapshotFileType::StoreBacked => {
                // For store-backed files, we need to re-create the symlink to the store
                // The store object should still exist (GC respects snapshots)
                if let (Some(hash), Some(drv_hash)) = (&file.hash, &file.derivation_hash) {
                    // Find the derivation in the snapshot and restore if it has an output path
                    if let Some(output_path) = snapshot
                        .derivations
                        .iter()
                        .find(|d| &d.hash == drv_hash)
                        .and_then(|drv| drv.output_path.as_ref())
                    {
                        // Re-create symlink
                        self.create_symlink(output_path, &file.path)?;
                        return Ok(true);
                    }

                    // Fallback: if we can't find the exact derivation, log a warning
                    warn!(
                        "Could not find derivation {} for file {} (hash: {})",
                        drv_hash,
                        file.path.display(),
                        hash
                    );
                }
                Ok(false)
            }
            SnapshotFileType::MutableSymlink => {
                // Re-create the symlink to the original source
                if let Some(target) = &file.target {
                    self.create_symlink(target, &file.path)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            SnapshotFileType::RegularFile => {
                // Restore from backup
                let backup_path = self.get_backup_path(&snapshot.id, &file.path);
                if backup_path.exists() {
                    // Ensure parent directory exists
                    if let Some(parent) = file.path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&backup_path, &file.path)?;
                    Ok(true)
                } else {
                    warn!(
                        "Backup not found for {}, cannot restore",
                        file.path.display()
                    );
                    Ok(false)
                }
            }
        }
    }

    /// Create a symlink, removing any existing file at the target.
    fn create_symlink(&self, source: &Path, target: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        // Remove existing file/symlink
        if target.is_symlink() || target.exists() {
            fs::remove_file(target)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(source, target)?;

        #[cfg(windows)]
        {
            if source.is_dir() {
                std::os::windows::fs::symlink_dir(source, target)?;
            } else {
                std::os::windows::fs::symlink_file(source, target)?;
            }
        }

        debug!(
            "Created symlink {} -> {}",
            target.display(),
            source.display()
        );
        Ok(())
    }

    /// Get the previous snapshot ID (for rollback).
    pub fn get_previous_snapshot_id(&self) -> Result<Option<String>> {
        let metadata = self.load_metadata()?;

        if metadata.snapshots.len() < 2 {
            return Ok(None);
        }

        // Find current index
        let current_id = metadata.current.as_deref();
        let current_idx = metadata
            .snapshots
            .iter()
            .position(|s| Some(s.id.as_str()) == current_id);

        match current_idx {
            Some(idx) if idx > 0 => Ok(Some(metadata.snapshots[idx - 1].id.clone())),
            _ => Ok(metadata.snapshots.last().map(|s| s.id.clone())),
        }
    }
}

/// Result of a rollback operation.
#[derive(Debug, Clone)]
pub struct RollbackResult {
    /// ID of the snapshot rolled back to
    pub target_id: String,
    /// Files that were restored
    pub files_restored: Vec<PathBuf>,
    /// Files that were removed
    pub files_removed: Vec<PathBuf>,
    /// Errors that occurred (rollback continues on errors)
    pub errors: Vec<String>,
}

impl RollbackResult {
    /// Check if the rollback was successful (no errors).
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get a summary of the rollback.
    pub fn summary(&self) -> String {
        format!(
            "Rolled back to {}: {} files restored, {} files removed, {} errors",
            self.target_id,
            self.files_restored.len(),
            self.files_removed.len(),
            self.errors.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (SnapshotManager, TempDir) {
        let temp = TempDir::new().unwrap();
        let manager = SnapshotManager::new(temp.path().join("snapshots"));
        manager.init().unwrap();
        (manager, temp)
    }

    #[test]
    fn test_snapshot_creation() {
        let snapshot = Snapshot::new("Test snapshot");

        assert!(!snapshot.id.is_empty());
        assert!(snapshot.created_at > 0);
        assert_eq!(snapshot.description, "Test snapshot");
        assert!(snapshot.files.is_empty());
        assert!(snapshot.envs.is_empty());
        assert!(snapshot.derivations.is_empty());
    }

    #[test]
    fn test_snapshot_with_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("init.lua");
        fs::write(&config_path, "file { path = '~/.test' }").unwrap();

        let snapshot = Snapshot::new("Test").with_config(&config_path);

        assert_eq!(snapshot.config_path.as_ref().unwrap(), &config_path);
        assert!(snapshot.config_content.as_ref().unwrap().contains("file"));
    }

    #[test]
    fn test_snapshot_manager_init() {
        let (manager, _temp) = setup_test_env();

        assert!(manager.snapshots_dir.exists());
        assert!(manager.files_dir.exists());
        assert!(manager.metadata_path.exists());
    }

    #[test]
    fn test_create_and_get_snapshot() {
        let (manager, _temp) = setup_test_env();

        let mut snapshot = Snapshot::new("First snapshot");
        snapshot.add_file(SnapshotFile::mutable_symlink(
            PathBuf::from("/home/user/.gitconfig"),
            PathBuf::from("/path/to/dotfiles/gitconfig"),
        ));

        let id = manager.create_snapshot(snapshot.clone()).unwrap();

        let loaded = manager.get_snapshot(&id).unwrap();
        assert_eq!(loaded.description, "First snapshot");
        assert_eq!(loaded.files.len(), 1);
    }

    #[test]
    fn test_list_snapshots() {
        let (manager, _temp) = setup_test_env();

        // Create multiple snapshots
        for i in 1..=3 {
            let snapshot = Snapshot::new(format!("Snapshot {}", i));
            manager.create_snapshot(snapshot).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different IDs
        }

        let list = manager.list_snapshots().unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_current_snapshot() {
        let (manager, _temp) = setup_test_env();

        // Initially no current
        assert!(manager.get_current_id().unwrap().is_none());

        // Create a snapshot
        let snapshot = Snapshot::new("Current test");
        let id = manager.create_snapshot(snapshot).unwrap();

        // Now we have a current
        assert_eq!(manager.get_current_id().unwrap(), Some(id));
    }

    #[test]
    fn test_delete_snapshot() {
        let (manager, _temp) = setup_test_env();

        let snapshot = Snapshot::new("To be deleted");
        let id = manager.create_snapshot(snapshot).unwrap();

        assert!(manager.get_snapshot(&id).is_ok());

        manager.delete_snapshot(&id).unwrap();

        assert!(manager.get_snapshot(&id).is_err());
        assert!(manager.list_snapshots().unwrap().is_empty());
    }

    #[test]
    fn test_snapshot_not_found() {
        let (manager, _temp) = setup_test_env();

        let result = manager.get_snapshot("nonexistent");
        assert!(matches!(result, Err(CoreError::SnapshotNotFound(_))));
    }

    #[test]
    fn test_backup_file() {
        let (manager, temp) = setup_test_env();

        // Create a file to backup
        let file_path = temp.path().join("test_file.txt");
        fs::write(&file_path, "Original content").unwrap();

        let snapshot_id = "test_snapshot_123";
        let backup_path = manager.backup_file(snapshot_id, &file_path).unwrap();

        assert!(backup_path.is_some());
        let backup_path = backup_path.unwrap();
        assert!(backup_path.exists());
        assert_eq!(
            fs::read_to_string(&backup_path).unwrap(),
            "Original content"
        );
    }

    #[test]
    fn test_get_previous_snapshot_id() {
        let (manager, _temp) = setup_test_env();

        // No snapshots - no previous
        assert!(manager.get_previous_snapshot_id().unwrap().is_none());

        // Create first snapshot
        let first = Snapshot::new("First");
        let first_id = manager.create_snapshot(first).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Still no previous (only one snapshot)
        assert!(manager.get_previous_snapshot_id().unwrap().is_none());

        // Create second snapshot
        let second = Snapshot::new("Second");
        let _second_id = manager.create_snapshot(second).unwrap();

        // Now first is the previous
        assert_eq!(manager.get_previous_snapshot_id().unwrap(), Some(first_id));
    }

    #[test]
    fn test_rollback_basic() {
        let (manager, temp) = setup_test_env();

        // Create a test file
        let test_file = temp.path().join("home/test.txt");
        fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        fs::write(&test_file, "Version 1").unwrap();

        // Create first snapshot with the file
        let mut snapshot1 = Snapshot::new("Version 1");
        snapshot1.add_file(SnapshotFile::mutable_symlink(
            test_file.clone(),
            PathBuf::from("/source/test.txt"),
        ));
        let id1 = manager.create_snapshot(snapshot1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create second snapshot without the file
        let snapshot2 = Snapshot::new("Version 2");
        manager.create_snapshot(snapshot2).unwrap();

        // Rollback to first snapshot
        let result = manager.rollback_to(&id1).unwrap();

        assert!(result.is_success());
        assert_eq!(manager.get_current_id().unwrap(), Some(id1));
    }

    #[test]
    fn test_snapshot_file_types() {
        let store_backed = SnapshotFile::store_backed(
            PathBuf::from("/home/user/.config"),
            "hash123".to_string(),
            "drv456".to_string(),
        );
        assert_eq!(store_backed.file_type, SnapshotFileType::StoreBacked);
        assert_eq!(store_backed.hash, Some("hash123".to_string()));

        let mutable = SnapshotFile::mutable_symlink(
            PathBuf::from("/home/user/.gitconfig"),
            PathBuf::from("/dotfiles/gitconfig"),
        );
        assert_eq!(mutable.file_type, SnapshotFileType::MutableSymlink);
        assert_eq!(mutable.target, Some(PathBuf::from("/dotfiles/gitconfig")));
    }

    #[test]
    fn test_snapshot_derivation() {
        let drv = SnapshotDerivation::new(
            "ripgrep".to_string(),
            Some("15.1.0".to_string()),
            "abc123".to_string(),
            "package",
        )
        .with_output(PathBuf::from("/store/obj/ripgrep-15.1.0-abc123"));

        assert_eq!(drv.name, "ripgrep");
        assert_eq!(drv.version, Some("15.1.0".to_string()));
        assert!(drv.output_path.is_some());
    }

    #[test]
    fn test_snapshot_env() {
        let env = SnapshotEnv::new("EDITOR".to_string(), "nvim".to_string(), "replace")
            .with_derivation("env123".to_string());

        assert_eq!(env.name, "EDITOR");
        assert_eq!(env.value, "nvim");
        assert_eq!(env.derivation_hash, Some("env123".to_string()));
    }

    #[test]
    fn test_rollback_result() {
        let result = RollbackResult {
            target_id: "123".to_string(),
            files_restored: vec![PathBuf::from("/file1"), PathBuf::from("/file2")],
            files_removed: vec![PathBuf::from("/file3")],
            errors: vec![],
        };

        assert!(result.is_success());
        assert!(result.summary().contains("2 files restored"));
        assert!(result.summary().contains("1 files removed"));
    }
}
