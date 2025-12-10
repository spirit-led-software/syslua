//! Declaration types collected from Lua config
//!
//! The sys.lua architecture has two core primitives:
//! - `derive {}` - produces store content through a build function
//! - `activate {}` - performs side effects during activation
//!
//! Higher-level constructs like `file {}` and `env {}` are built on these primitives.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// =============================================================================
// Core Primitives: derive {} and activate {}
// =============================================================================

/// A derive declaration from the Lua config.
///
/// `derive {}` is one of the two core primitives. It describes how to produce
/// store content through a build function.
///
/// ```lua
/// local drv = derive {
///     name = "ripgrep",
///     version = "15.1.0",
///
///     opts = function(sys)
///         return {
///             url = "https://github.com/.../ripgrep-" .. sys.platform .. ".tar.gz",
///             sha256 = "abc123...",
///         }
///     end,
///
///     config = function(opts, ctx)
///         local archive = ctx.fetch_url(opts.url, opts.sha256)
///         ctx.unpack(archive, ctx.out)
///     end,
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeriveDecl {
    /// Name of the derivation (for display/debugging)
    pub name: String,

    /// Optional version string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Resolved options (after calling opts function with sys)
    pub opts: BTreeMap<String, DeriveInput>,

    /// Hash of the config function source code
    pub config_hash: String,

    /// Output names (defaults to ["out"])
    #[serde(default = "default_outputs")]
    pub outputs: Vec<String>,

    /// Platform this derivation was evaluated for
    pub platform: String,
}

fn default_outputs() -> Vec<String> {
    vec!["out".to_string()]
}

impl DeriveDecl {
    /// Create a new derive declaration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            opts: BTreeMap::new(),
            config_hash: String::new(),
            outputs: vec!["out".to_string()],
            platform: String::new(),
        }
    }

    /// Compute a deterministic hash for this derivation specification.
    ///
    /// The hash is computed from: name, version, opts, config_hash, outputs, platform.
    /// This determines cache hits - same hash = same output.
    pub fn compute_hash(&self) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();

        hasher.update(b"name:");
        hasher.update(self.name.as_bytes());
        hasher.update(b"\n");

        if let Some(v) = &self.version {
            hasher.update(b"version:");
            hasher.update(v.as_bytes());
            hasher.update(b"\n");
        }

        hasher.update(b"opts:");
        if let Ok(opts_json) = serde_json::to_string(&self.opts) {
            hasher.update(opts_json.as_bytes());
        }
        hasher.update(b"\n");

        hasher.update(b"config:");
        hasher.update(self.config_hash.as_bytes());
        hasher.update(b"\n");

        hasher.update(b"outputs:");
        hasher.update(self.outputs.join(",").as_bytes());
        hasher.update(b"\n");

        hasher.update(b"platform:");
        hasher.update(self.platform.as_bytes());

        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Get a short hash for display (first 12 chars)
    pub fn short_hash(&self) -> String {
        self.compute_hash()[..12].to_string()
    }
}

/// Input value types for derive declarations.
///
/// These represent the resolved values from the `opts` function.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DeriveInput {
    /// A string value
    String(String),
    /// A numeric value
    Number(f64),
    /// A boolean value
    Bool(bool),
    /// A nested table/map
    Table(BTreeMap<String, DeriveInput>),
    /// An array of values
    Array(Vec<DeriveInput>),
    /// A reference to another derivation (by hash)
    DeriveRef(DeriveRef),
}

/// Reference to another derivation's output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeriveRef {
    /// The derivation hash
    pub hash: String,
    /// The derivation name (for display)
    pub name: String,
    /// Which output to reference (defaults to "out")
    #[serde(default = "default_out")]
    pub output: String,
}

fn default_out() -> String {
    "out".to_string()
}

impl DeriveRef {
    /// Create a new derivation reference
    pub fn new(hash: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            hash: hash.into(),
            name: name.into(),
            output: "out".to_string(),
        }
    }

    /// Create a reference to a specific output
    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = output.into();
        self
    }
}

/// An activate declaration from the Lua config.
///
/// `activate {}` is the second core primitive. It describes side effects to perform
/// during activation (symlinking, PATH modifications, service registration, etc.).
///
/// ```lua
/// activate {
///     opts = function(sys)
///         return { drv = some_derivation }
///     end,
///
///     config = function(opts, ctx)
///         ctx.add_to_path(opts.drv.out .. "/bin")
///         ctx.symlink(opts.drv.out .. "/share/man", "~/.local/share/man/ripgrep")
///     end,
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivateDecl {
    /// Optional name for debugging/display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Resolved options (after calling opts function with sys)
    pub opts: BTreeMap<String, ActivateInput>,

    /// Hash of the config function source code
    pub config_hash: String,

    /// Derivation references this activation depends on
    pub dependencies: Vec<DeriveRef>,

    /// Actions collected during config function execution
    #[serde(default)]
    pub actions: Vec<ActivateAction>,
}

impl ActivateDecl {
    /// Create a new activate declaration
    pub fn new() -> Self {
        Self {
            name: None,
            opts: BTreeMap::new(),
            config_hash: String::new(),
            dependencies: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Create a named activate declaration
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::new()
        }
    }
}

impl Default for ActivateDecl {
    fn default() -> Self {
        Self::new()
    }
}

/// Input value types for activate declarations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ActivateInput {
    /// A string value
    String(String),
    /// A numeric value
    Number(f64),
    /// A boolean value
    Bool(bool),
    /// A nested table/map
    Table(BTreeMap<String, ActivateInput>),
    /// An array of values
    Array(Vec<ActivateInput>),
    /// A reference to a derivation
    DeriveRef(DeriveRef),
}

/// Actions that can be performed during activation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum ActivateAction {
    /// Add a directory to PATH
    AddToPath {
        /// Path to add (usually drv.out/bin)
        path: String,
    },
    /// Create a symlink
    Symlink {
        /// Source path (in store)
        source: String,
        /// Target path (in user's filesystem)
        target: String,
        /// Whether to create parent directories
        #[serde(default)]
        mkdir: bool,
    },
    /// Set an environment variable
    SetEnv {
        /// Variable name
        name: String,
        /// Variable value
        value: String,
    },
    /// Source a shell script in the activation script
    SourceScript {
        /// Path to the script
        path: String,
        /// Which shells to source in (bash, zsh, fish, etc.)
        #[serde(default)]
        shells: Vec<String>,
    },
    /// Run a command (escape hatch - use sparingly)
    Run {
        /// Command to run
        command: String,
        /// Arguments
        #[serde(default)]
        args: Vec<String>,
    },
}

// =============================================================================
// Higher-Level Declarations (built on derive/activate)
// =============================================================================

/// A file declaration from the Lua config.
///
/// Files are a convenience layer over derive/activate. They create a derivation
/// for the file content and an activation to symlink it to the target path.
///
/// ```lua
/// file { path = "~/.gitconfig", source = "./dotfiles/gitconfig" }
/// file { path = "~/.config/nvim/init.lua", content = [[require("config")]] }
/// file { path = "~/.gitconfig", source = "./dotfiles/gitconfig", mutable = true }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDecl {
    /// Target path for the file (with ~ expanded)
    pub path: PathBuf,

    /// Source file path - content goes to store, symlink to path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<PathBuf>,

    /// Inline file content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Whether this is a mutable file (direct symlink, not store-backed)
    /// Only applies when `source` is set
    #[serde(default)]
    pub mutable: bool,

    /// Unix file permissions (e.g., 0o755)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

impl FileDecl {
    /// Create a new store-backed file from source
    pub fn from_source(path: impl Into<PathBuf>, source: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            source: Some(source.into()),
            content: None,
            mutable: false,
            mode: None,
        }
    }

    /// Create a new file with inline content
    pub fn from_content(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: None,
            content: Some(content.into()),
            mutable: false,
            mode: None,
        }
    }

    /// Create a new mutable file (direct symlink)
    pub fn mutable_source(path: impl Into<PathBuf>, source: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            source: Some(source.into()),
            content: None,
            mutable: true,
            mode: None,
        }
    }

    /// Validate that the file declaration is valid
    pub fn validate(&self) -> Result<(), String> {
        let source_count = [self.source.is_some(), self.content.is_some()]
            .iter()
            .filter(|&&x| x)
            .count();

        if source_count == 0 {
            return Err(format!(
                "File declaration for '{}' must specify either source or content",
                self.path.display()
            ));
        }

        if source_count > 1 {
            return Err(format!(
                "File declaration for '{}' cannot specify both source and content",
                self.path.display()
            ));
        }

        // mutable only makes sense with source
        if self.mutable && self.source.is_none() {
            return Err(format!(
                "File declaration for '{}': mutable can only be used with source",
                self.path.display()
            ));
        }

        Ok(())
    }

    /// Get a description of the file type for display
    pub fn kind(&self) -> &'static str {
        if self.source.is_some() {
            if self.mutable {
                "mutable"
            } else {
                "source"
            }
        } else if self.content.is_some() {
            "content"
        } else {
            "unknown"
        }
    }
}

/// How to handle a PATH-like environment variable
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EnvMergeStrategy {
    /// Replace any existing value
    #[default]
    Replace,
    /// Prepend to existing PATH-like variable
    Prepend,
    /// Append to existing PATH-like variable
    Append,
}

/// A single environment variable value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvValue {
    /// The value to set
    pub value: String,
    /// How to merge with existing value (for PATH-like vars)
    #[serde(default)]
    pub strategy: EnvMergeStrategy,
}

impl EnvValue {
    /// Create a new replace-style env value
    pub fn replace(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            strategy: EnvMergeStrategy::Replace,
        }
    }

    /// Create a new prepend-style env value (for PATH-like vars)
    pub fn prepend(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            strategy: EnvMergeStrategy::Prepend,
        }
    }

    /// Create a new append-style env value (for PATH-like vars)
    pub fn append(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            strategy: EnvMergeStrategy::Append,
        }
    }
}

/// An environment variable declaration from the Lua config.
///
/// Represents an environment variable that sys.lua should manage.
///
/// ```lua
/// env {
///     EDITOR = "nvim",
///     PATH = { "~/.local/bin" },  -- prepend
///     MANPATH = { append = "/usr/share/man" },
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvDecl {
    /// Environment variable name
    pub name: String,
    /// Values to set (multiple for PATH-like prepend/append)
    pub values: Vec<EnvValue>,
}

impl EnvDecl {
    /// Create a new environment variable with a single replace value
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: vec![EnvValue::replace(value)],
        }
    }

    /// Create a new PATH-like environment variable with prepend values
    pub fn path_prepend(name: impl Into<String>, paths: Vec<String>) -> Self {
        Self {
            name: name.into(),
            values: paths.into_iter().map(EnvValue::prepend).collect(),
        }
    }

    /// Check if this is a PATH-like variable (has prepend/append values)
    pub fn is_path_like(&self) -> bool {
        self.values.iter().any(|v| {
            matches!(
                v.strategy,
                EnvMergeStrategy::Prepend | EnvMergeStrategy::Append
            )
        })
    }
}

/// An input declaration from the Lua config.
///
/// Inputs are external sources of Lua code (packages, libraries, etc.)
///
/// ```lua
/// local pkgs = input { source = "sys-lua/pkgs" }           -- GitHub: owner/repo
/// local pkgs_v2 = input { source = "sys-lua/pkgs/v2.0.0" } -- GitHub: owner/repo/ref
/// local local_pkgs = input { source = "path:./my-packages" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputDecl {
    /// Unique identifier for this input (generated)
    pub id: String,

    /// Input source URI:
    /// - GitHub: "owner/repo" or "owner/repo/ref"
    /// - Local: "path:./relative" or "path:/absolute"
    pub source: String,

    /// Resolved local path (set after resolution)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<PathBuf>,
}

impl InputDecl {
    /// Create a new input declaration
    pub fn new(id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            resolved_path: None,
        }
    }

    /// Set the resolved local path
    pub fn with_resolved_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.resolved_path = Some(path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_decl_hash() {
        let decl = DeriveDecl {
            name: "ripgrep".to_string(),
            version: Some("15.1.0".to_string()),
            opts: BTreeMap::new(),
            config_hash: "abc123".to_string(),
            outputs: vec!["out".to_string()],
            platform: "aarch64-darwin".to_string(),
        };

        let hash = decl.compute_hash();
        assert_eq!(hash.len(), 64); // SHA-256 hex

        // Same decl should produce same hash
        let hash2 = decl.compute_hash();
        assert_eq!(hash, hash2);

        // Different decl should produce different hash
        let mut decl2 = decl.clone();
        decl2.version = Some("15.2.0".to_string());
        let hash3 = decl2.compute_hash();
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_derive_ref() {
        let r = DeriveRef::new("abc123", "ripgrep");
        assert_eq!(r.output, "out");

        let r2 = r.clone().with_output("bin");
        assert_eq!(r2.output, "bin");
    }

    #[test]
    fn test_file_decl_validate() {
        // Valid source
        let decl = FileDecl::from_source("/path", "./source");
        assert!(decl.validate().is_ok());

        // Valid content
        let decl = FileDecl::from_content("/path", "content");
        assert!(decl.validate().is_ok());

        // Invalid: neither source nor content
        let decl = FileDecl {
            path: PathBuf::from("/path"),
            source: None,
            content: None,
            mutable: false,
            mode: None,
        };
        assert!(decl.validate().is_err());

        // Invalid: mutable without source
        let decl = FileDecl {
            path: PathBuf::from("/path"),
            source: None,
            content: Some("x".to_string()),
            mutable: true,
            mode: None,
        };
        assert!(decl.validate().is_err());
    }

    #[test]
    fn test_env_decl() {
        let decl = EnvDecl::new("EDITOR", "nvim");
        assert!(!decl.is_path_like());

        let decl = EnvDecl::path_prepend("PATH", vec!["~/.local/bin".to_string()]);
        assert!(decl.is_path_like());
    }

    #[test]
    fn test_activate_action_serialization() {
        let action = ActivateAction::AddToPath {
            path: "/usr/local/bin".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("AddToPath"));

        let action = ActivateAction::Symlink {
            source: "/store/abc".to_string(),
            target: "~/.config".to_string(),
            mkdir: true,
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("Symlink"));
    }
}
