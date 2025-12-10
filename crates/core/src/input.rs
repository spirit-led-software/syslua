//! Input handling for sys.lua
//!
//! Inputs are external sources of Lua code and derivations. They can be:
//! - GitHub repositories (`owner/repo` or `owner/repo/ref`)
//! - Local paths (`path:./relative/path` or `path:/absolute/path`)
//!
//! # Example
//!
//! ```lua
//! -- inputs.lua
//! local M = {}
//!
//! M.pkgs = input { source = "sys-lua/pkgs" }           -- defaults to main
//! M.pkgs_v2 = input { source = "sys-lua/pkgs/v2.0.0" } -- specific tag
//! M.local_pkgs = input { source = "path:./my-packages" }
//!
//! return M
//!
//! -- init.lua
//! local inputs = require("inputs")
//! pkg(inputs.pkgs.ripgrep)  -- loads ripgrep from pkgs repo's init.lua exports
//! ```
//!
//! # Lock File
//!
//! Inputs are recorded in `syslua.lock` for reproducibility. The lock file
//! contains the resolved commit/revision for each input.

use crate::Result;
use crate::error::CoreError;
use crate::store::sha256_string;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// The type of input source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InputSource {
    /// A GitHub repository.
    GitHub {
        /// Repository owner.
        owner: String,
        /// Repository name.
        repo: String,
        /// Git reference (branch, tag, or commit). Defaults to "main".
        #[serde(rename = "ref", default = "default_github_ref")]
        git_ref: String,
    },
    /// A local path.
    Path {
        /// The path (relative or absolute).
        path: PathBuf,
    },
}

fn default_github_ref() -> String {
    "main".to_string()
}

impl InputSource {
    /// Parse an input URI string.
    ///
    /// Supported formats:
    /// - `owner/repo` (GitHub, defaults to main branch)
    /// - `owner/repo/ref` (GitHub, specific branch/tag/commit)
    /// - `path:./relative/path` (local path)
    /// - `path:/absolute/path` (local path)
    pub fn parse(uri: &str) -> Result<Self> {
        // Local paths use path: prefix
        if let Some(rest) = uri.strip_prefix("path:") {
            return Ok(Self::Path {
                path: PathBuf::from(rest),
            });
        }

        // Everything else is GitHub: owner/repo or owner/repo/ref
        Self::parse_github(uri)
    }

    /// Parse a GitHub input (owner/repo or owner/repo/ref format).
    fn parse_github(uri: &str) -> Result<Self> {
        let parts: Vec<&str> = uri.split('/').collect();
        match parts.len() {
            2 => Ok(Self::GitHub {
                owner: parts[0].to_string(),
                repo: parts[1].to_string(),
                git_ref: default_github_ref(),
            }),
            3 => Ok(Self::GitHub {
                owner: parts[0].to_string(),
                repo: parts[1].to_string(),
                git_ref: parts[2].to_string(),
            }),
            _ => Err(CoreError::InvalidInput(format!(
                "Invalid input: '{}'. Expected 'owner/repo', 'owner/repo/ref', or 'path:./local'",
                uri
            ))),
        }
    }

    /// Get a unique identifier for this input source.
    pub fn id(&self) -> String {
        match self {
            Self::GitHub {
                owner,
                repo,
                git_ref,
            } => format!("github-{}-{}-{}", owner, repo, git_ref),
            Self::Path { path } => format!("path-{}", sha256_string(&path.display().to_string())),
        }
    }

    /// Get the URI representation of this input.
    pub fn to_uri(&self) -> String {
        match self {
            Self::GitHub {
                owner,
                repo,
                git_ref,
            } => {
                if git_ref == "main" {
                    format!("{}/{}", owner, repo)
                } else {
                    format!("{}/{}/{}", owner, repo, git_ref)
                }
            }
            Self::Path { path } => format!("path:{}", path.display()),
        }
    }
}

/// A resolved input with its local path and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedInput {
    /// The original input source.
    pub source: InputSource,
    /// The local path where the input is available.
    pub local_path: PathBuf,
    /// The resolved revision (commit SHA for GitHub, or None for local paths).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// When this input was last fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
}

impl ResolvedInput {
    /// Create a resolved input from a local path.
    pub fn from_local_path(source: InputSource, path: PathBuf) -> Self {
        Self {
            source,
            local_path: path,
            revision: None,
            fetched_at: None,
        }
    }

    /// Create a resolved input from a fetched source.
    pub fn from_fetched(source: InputSource, path: PathBuf, revision: String) -> Self {
        Self {
            source,
            local_path: path,
            revision: Some(revision),
            fetched_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }
}

/// Lock file format for reproducible builds.
///
/// The lock file records the resolved state of all inputs, including
/// commit SHAs for GitHub inputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockFile {
    /// Version of the lock file format.
    pub version: u32,
    /// Map of input name to locked input.
    pub inputs: BTreeMap<String, LockedInput>,
}

/// A locked input entry in the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedInput {
    /// The input URI (e.g., "github:owner/repo/ref").
    pub uri: String,
    /// The input source type and details.
    pub source: InputSource,
    /// The resolved revision (commit SHA for GitHub).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Content hash of the input (for integrity verification).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// When this input was last updated.
    pub updated_at: String,
}

impl LockFile {
    /// Current lock file format version.
    pub const VERSION: u32 = 1;

    /// Create a new empty lock file.
    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            inputs: BTreeMap::new(),
        }
    }

    /// Load a lock file from disk.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path)?;
        let lock: Self = serde_json::from_str(&content)
            .map_err(|e| CoreError::InvalidInput(format!("Failed to parse lock file: {}", e)))?;

        if lock.version > Self::VERSION {
            return Err(CoreError::InvalidInput(format!(
                "Lock file version {} is newer than supported version {}",
                lock.version,
                Self::VERSION
            )));
        }

        Ok(lock)
    }

    /// Save the lock file to disk.
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;

        // Write atomically
        let temp_path = path.with_extension("lock.tmp");
        fs::write(&temp_path, &content)?;
        fs::rename(&temp_path, path)?;

        info!("Saved lock file to {}", path.display());
        Ok(())
    }

    /// Get a locked input by name.
    pub fn get(&self, name: &str) -> Option<&LockedInput> {
        self.inputs.get(name)
    }

    /// Add or update a locked input.
    pub fn set(&mut self, name: String, locked: LockedInput) {
        self.inputs.insert(name, locked);
    }

    /// Check if an input needs updating.
    ///
    /// Returns true if:
    /// - The input is not in the lock file
    /// - The input URI has changed
    pub fn needs_update(&self, name: &str, source: &InputSource) -> bool {
        match self.inputs.get(name) {
            None => true,
            Some(locked) => locked.uri != source.to_uri(),
        }
    }
}

/// Input manager handles fetching and caching inputs.
#[derive(Debug)]
pub struct InputManager {
    /// Directory where inputs are cached.
    cache_dir: PathBuf,
    /// Lock file for reproducibility.
    lock_file: LockFile,
    /// Path to the lock file on disk.
    lock_path: PathBuf,
}

impl InputManager {
    /// Create a new input manager.
    pub fn new(cache_dir: PathBuf, lock_path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir)?;
        let lock_file = LockFile::load(&lock_path)?;

        Ok(Self {
            cache_dir,
            lock_file,
            lock_path,
        })
    }

    /// Get the cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get the lock file.
    pub fn lock_file(&self) -> &LockFile {
        &self.lock_file
    }

    /// Save the current lock file state.
    pub fn save_lock_file(&self) -> Result<()> {
        self.lock_file.save(&self.lock_path)
    }

    /// Resolve an input, fetching if necessary.
    ///
    /// If `update` is true, fetches the latest version even if cached.
    /// Otherwise, uses the locked version if available.
    pub fn resolve(
        &mut self,
        name: &str,
        source: &InputSource,
        update: bool,
    ) -> Result<ResolvedInput> {
        match source {
            InputSource::Path { path } => self.resolve_local(name, path),
            InputSource::GitHub { .. } => self.resolve_github(name, source, update),
        }
    }

    /// Resolve a local path input.
    fn resolve_local(&self, name: &str, path: &Path) -> Result<ResolvedInput> {
        let resolved_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            // Relative paths are resolved from current directory
            std::env::current_dir()?.join(path)
        };

        if !resolved_path.exists() {
            return Err(CoreError::InvalidInput(format!(
                "Local input '{}' not found: {}",
                name,
                resolved_path.display()
            )));
        }

        debug!(
            "Resolved local input '{}' to {}",
            name,
            resolved_path.display()
        );

        Ok(ResolvedInput::from_local_path(
            InputSource::Path {
                path: path.to_path_buf(),
            },
            resolved_path,
        ))
    }

    /// Resolve a GitHub input.
    fn resolve_github(
        &mut self,
        name: &str,
        source: &InputSource,
        update: bool,
    ) -> Result<ResolvedInput> {
        let InputSource::GitHub {
            owner,
            repo,
            git_ref,
        } = source
        else {
            unreachable!()
        };

        // Check if we have a locked version
        let locked = self.lock_file.get(name);
        let use_locked = !update && locked.is_some() && !self.lock_file.needs_update(name, source);

        if use_locked {
            let locked = locked.unwrap();
            let cache_path =
                self.github_cache_path(owner, repo, locked.revision.as_deref().unwrap_or(git_ref));

            if cache_path.exists() {
                debug!(
                    "Using cached input '{}' from {}",
                    name,
                    cache_path.display()
                );
                return Ok(ResolvedInput {
                    source: source.clone(),
                    local_path: cache_path,
                    revision: locked.revision.clone(),
                    fetched_at: Some(locked.updated_at.clone()),
                });
            }
        }

        // Fetch from GitHub
        let (cache_path, revision) = self.fetch_github(owner, repo, git_ref)?;

        // Update lock file
        let locked_input = LockedInput {
            uri: source.to_uri(),
            source: source.clone(),
            revision: Some(revision.clone()),
            hash: None, // TODO: compute hash of downloaded content
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        self.lock_file.set(name.to_string(), locked_input);

        info!(
            "Fetched input '{}' from GitHub ({}/{}@{})",
            name,
            owner,
            repo,
            &revision[..8.min(revision.len())]
        );

        Ok(ResolvedInput::from_fetched(
            source.clone(),
            cache_path,
            revision,
        ))
    }

    /// Get the cache path for a GitHub repository.
    fn github_cache_path(&self, owner: &str, repo: &str, revision: &str) -> PathBuf {
        self.cache_dir.join(format!(
            "github-{}-{}-{}",
            owner,
            repo,
            &revision[..12.min(revision.len())]
        ))
    }

    /// Fetch a GitHub repository tarball.
    fn fetch_github(&self, owner: &str, repo: &str, git_ref: &str) -> Result<(PathBuf, String)> {
        // First, resolve the ref to a commit SHA using the GitHub API
        let commit_sha = self.resolve_github_ref(owner, repo, git_ref)?;

        let cache_path = self.github_cache_path(owner, repo, &commit_sha);

        // Check if already cached
        if cache_path.exists() {
            debug!("GitHub input already cached at {}", cache_path.display());
            return Ok((cache_path, commit_sha));
        }

        // Download tarball
        let tarball_url = format!(
            "https://github.com/{}/{}/archive/{}.tar.gz",
            owner, repo, commit_sha
        );

        info!("Downloading {} ...", tarball_url);

        // Use blocking reqwest for simplicity (we're in sync code)
        let response = reqwest::blocking::get(&tarball_url).map_err(|e| {
            CoreError::NetworkError(format!("Failed to download {}: {}", tarball_url, e))
        })?;

        if !response.status().is_success() {
            return Err(CoreError::NetworkError(format!(
                "Failed to download {}: HTTP {}",
                tarball_url,
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .map_err(|e| CoreError::NetworkError(format!("Failed to read response: {}", e)))?;

        // Extract tarball
        let temp_dir = tempfile::tempdir()?;
        let tar_gz = flate2::read::GzDecoder::new(&bytes[..]);
        let mut archive = tar::Archive::new(tar_gz);
        archive.unpack(temp_dir.path())?;

        // The tarball extracts to a directory like "repo-commitsha/"
        // Find and move it to the cache location
        let entries: Vec<_> = fs::read_dir(temp_dir.path())?
            .filter_map(|e| e.ok())
            .collect();

        if entries.len() != 1 {
            return Err(CoreError::InvalidInput(format!(
                "Expected single directory in tarball, found {}",
                entries.len()
            )));
        }

        let extracted_dir = entries[0].path();

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Move to cache location
        fs::rename(&extracted_dir, &cache_path)?;

        Ok((cache_path, commit_sha))
    }

    /// Resolve a GitHub ref (branch/tag) to a commit SHA.
    fn resolve_github_ref(&self, owner: &str, repo: &str, git_ref: &str) -> Result<String> {
        // If it looks like a full SHA, use it directly
        if git_ref.len() == 40 && git_ref.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(git_ref.to_string());
        }

        // Use GitHub API to resolve the ref
        let api_url = format!(
            "https://api.github.com/repos/{}/{}/commits/{}",
            owner, repo, git_ref
        );

        debug!("Resolving GitHub ref via {}", api_url);

        let client = reqwest::blocking::Client::new();
        let response = client
            .get(&api_url)
            .header("User-Agent", "sys-lua")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| CoreError::NetworkError(format!("Failed to resolve ref: {}", e)))?;

        if !response.status().is_success() {
            return Err(CoreError::NetworkError(format!(
                "Failed to resolve ref '{}' for {}/{}: HTTP {}",
                git_ref,
                owner,
                repo,
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct CommitResponse {
            sha: String,
        }

        let commit: CommitResponse = response.json().map_err(|e| {
            CoreError::NetworkError(format!("Failed to parse GitHub response: {}", e))
        })?;

        Ok(commit.sha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_github_input() {
        // owner/repo defaults to main
        let source = InputSource::parse("owner/repo").unwrap();
        assert!(matches!(
            source,
            InputSource::GitHub { owner, repo, git_ref }
            if owner == "owner" && repo == "repo" && git_ref == "main"
        ));

        // owner/repo/ref with specific ref
        let source = InputSource::parse("sys-lua/pkgs/v1.0.0").unwrap();
        assert!(matches!(
            source,
            InputSource::GitHub { owner, repo, git_ref }
            if owner == "sys-lua" && repo == "pkgs" && git_ref == "v1.0.0"
        ));
    }

    #[test]
    fn test_parse_path_input() {
        let source = InputSource::parse("path:./local/packages").unwrap();
        assert!(matches!(
            source,
            InputSource::Path { path } if path == std::path::Path::new("./local/packages")
        ));

        let source = InputSource::parse("path:/absolute/path").unwrap();
        assert!(matches!(
            source,
            InputSource::Path { path } if path == std::path::Path::new("/absolute/path")
        ));
    }

    #[test]
    fn test_parse_invalid_input() {
        // Single segment is invalid (not owner/repo)
        assert!(InputSource::parse("owner").is_err());
        // Too many segments
        assert!(InputSource::parse("a/b/c/d").is_err());
    }

    #[test]
    fn test_input_source_id() {
        let github = InputSource::GitHub {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            git_ref: "main".to_string(),
        };
        assert_eq!(github.id(), "github-owner-repo-main");

        let path = InputSource::Path {
            path: PathBuf::from("./local"),
        };
        assert!(path.id().starts_with("path-"));
    }

    #[test]
    fn test_input_source_to_uri() {
        let github = InputSource::GitHub {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            git_ref: "main".to_string(),
        };
        assert_eq!(github.to_uri(), "owner/repo");

        let github_ref = InputSource::GitHub {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            git_ref: "v1.0.0".to_string(),
        };
        assert_eq!(github_ref.to_uri(), "owner/repo/v1.0.0");
    }

    #[test]
    fn test_lock_file_new() {
        let lock = LockFile::new();
        assert_eq!(lock.version, LockFile::VERSION);
        assert!(lock.inputs.is_empty());
    }

    #[test]
    fn test_lock_file_save_load() {
        let temp = TempDir::new().unwrap();
        let lock_path = temp.path().join("syslua.lock");

        let mut lock = LockFile::new();
        lock.set(
            "test".to_string(),
            LockedInput {
                uri: "owner/repo".to_string(),
                source: InputSource::GitHub {
                    owner: "owner".to_string(),
                    repo: "repo".to_string(),
                    git_ref: "main".to_string(),
                },
                revision: Some("abc123".to_string()),
                hash: None,
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
        );

        lock.save(&lock_path).unwrap();
        assert!(lock_path.exists());

        let loaded = LockFile::load(&lock_path).unwrap();
        assert_eq!(loaded.version, lock.version);
        assert!(loaded.get("test").is_some());
        assert_eq!(
            loaded.get("test").unwrap().revision,
            Some("abc123".to_string())
        );
    }

    #[test]
    fn test_lock_file_needs_update() {
        let mut lock = LockFile::new();

        let source = InputSource::GitHub {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            git_ref: "main".to_string(),
        };

        // Not in lock file - needs update
        assert!(lock.needs_update("test", &source));

        // Add to lock file
        lock.set(
            "test".to_string(),
            LockedInput {
                uri: source.to_uri(),
                source: source.clone(),
                revision: Some("abc123".to_string()),
                hash: None,
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
        );

        // Now in lock file with same URI - no update needed
        assert!(!lock.needs_update("test", &source));

        // Different URI - needs update
        let new_source = InputSource::GitHub {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            git_ref: "v2.0.0".to_string(),
        };
        assert!(lock.needs_update("test", &new_source));
    }

    #[test]
    fn test_input_manager_resolve_local() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().join("cache");
        let lock_path = temp.path().join("syslua.lock");

        // Create a local input directory
        let local_dir = temp.path().join("my-packages");
        fs::create_dir_all(&local_dir).unwrap();
        fs::write(local_dir.join("test.lua"), "return {}").unwrap();

        let mut manager = InputManager::new(cache_dir, lock_path).unwrap();

        let source = InputSource::Path {
            path: local_dir.clone(),
        };
        let resolved = manager.resolve("local", &source, false).unwrap();

        assert_eq!(resolved.local_path, local_dir);
        assert!(resolved.revision.is_none());
    }

    #[test]
    fn test_input_manager_local_not_found() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().join("cache");
        let lock_path = temp.path().join("syslua.lock");

        let mut manager = InputManager::new(cache_dir, lock_path).unwrap();

        let source = InputSource::Path {
            path: PathBuf::from("/nonexistent/path"),
        };

        let result = manager.resolve("local", &source, false);
        assert!(result.is_err());
    }
}
