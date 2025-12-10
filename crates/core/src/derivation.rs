//! Derivation types for sys.lua
//!
//! A derivation is the sole primitive for producing store content. It describes:
//! - What inputs are needed (arbitrary data)
//! - How to transform those inputs into outputs (build function)
//! - What outputs are produced

use crate::store::{sha256_string, truncate_hash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Input value types that can appear in a derivation's inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputValue {
    /// A string value
    String(String),
    /// A numeric value
    Number(f64),
    /// A boolean value
    Bool(bool),
    /// A nested table/map
    Table(BTreeMap<String, InputValue>),
    /// An array of values
    Array(Vec<InputValue>),
    /// A reference to another derivation (by hash)
    DerivationRef(DerivationRef),
}

/// Reference to another derivation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivationRef {
    /// The derivation hash
    pub hash: String,
    /// Output paths (available after realization)
    pub outputs: BTreeMap<String, PathBuf>,
}

/// Platform/system information passed to inputs function and build function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct System {
    /// Platform identifier (e.g., "aarch64-darwin", "x86_64-linux")
    pub platform: String,
    /// Operating system
    pub os: String,
    /// CPU architecture
    pub arch: String,
    /// Machine hostname
    pub hostname: String,
    /// Current user
    pub username: String,
}

impl System {
    /// Create a System from the current platform.
    ///
    /// Returns a default system with "unknown" values if platform detection fails.
    pub fn current() -> Self {
        match sys_platform::Platform::detect() {
            Ok(platform) => Self {
                platform: platform.platform.clone(),
                os: platform.os.as_str().to_string(),
                arch: platform.arch.as_str().to_string(),
                hostname: platform.hostname.clone(),
                username: platform.username.clone(),
            },
            Err(_) => Self {
                platform: "unknown".to_string(),
                os: "unknown".to_string(),
                arch: "unknown".to_string(),
                hostname: "unknown".to_string(),
                username: "unknown".to_string(),
            },
        }
    }
}

/// The specification for a derivation.
///
/// This is the immutable description of how to produce content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationSpec {
    /// Name of the derivation (for debugging/logging)
    pub name: String,

    /// Optional version string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Resolved inputs (after evaluating inputs function with system)
    pub inputs: BTreeMap<String, InputValue>,

    /// Hash of the build function source code
    pub build_hash: String,

    /// Output names (defaults to ["out"])
    #[serde(default = "default_outputs")]
    pub outputs: Vec<String>,

    /// System information this derivation was evaluated for
    pub system: System,
}

fn default_outputs() -> Vec<String> {
    vec!["out".to_string()]
}

impl DerivationSpec {
    /// Compute the derivation hash from its specification.
    ///
    /// The hash is computed from:
    /// - name
    /// - version (if present)
    /// - inputs (serialized)
    /// - build function hash
    /// - outputs
    /// - system
    pub fn compute_hash(&self) -> String {
        // Use a stable serialization for hashing
        let hash_input = format!(
            "name:{}\nversion:{}\ninputs:{}\nbuild:{}\noutputs:{}\nsystem:{}",
            self.name,
            self.version.as_deref().unwrap_or(""),
            serde_json::to_string(&self.inputs).unwrap_or_default(),
            self.build_hash,
            self.outputs.join(","),
            serde_json::to_string(&self.system).unwrap_or_default(),
        );
        sha256_string(&hash_input)
    }
}

/// A realized derivation with computed hash and output paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Derivation {
    /// The derivation specification
    pub spec: DerivationSpec,

    /// Computed derivation hash
    pub hash: String,

    /// Output paths (populated after realization)
    /// Maps output name -> store path
    #[serde(default)]
    pub output_paths: BTreeMap<String, PathBuf>,

    /// Whether this derivation has been realized
    #[serde(default)]
    pub realized: bool,
}

impl Derivation {
    /// Create a new derivation from a specification.
    pub fn new(spec: DerivationSpec) -> Self {
        let hash = spec.compute_hash();
        Self {
            spec,
            hash,
            output_paths: BTreeMap::new(),
            realized: false,
        }
    }

    /// Get the derivation name.
    pub fn name(&self) -> &str {
        &self.spec.name
    }

    /// Get the derivation version.
    pub fn version(&self) -> Option<&str> {
        self.spec.version.as_deref()
    }

    /// Get the truncated hash for display.
    pub fn short_hash(&self) -> &str {
        truncate_hash(&self.hash)
    }

    /// Get the primary output path ("out").
    pub fn out(&self) -> Option<&PathBuf> {
        self.output_paths.get("out")
    }

    /// Get a specific output path.
    pub fn output(&self, name: &str) -> Option<&PathBuf> {
        self.output_paths.get(name)
    }
}

/// A link registration that connects a derivation output to a target path.
///
/// This is used for files and packages that should appear at specific locations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRegistration {
    /// The derivation hash
    pub derivation_hash: String,

    /// Output name (usually "out")
    pub output: String,

    /// Target path where the link should be created
    pub target: PathBuf,

    /// Whether this is a mutable link (direct symlink, not through store)
    #[serde(default)]
    pub mutable: bool,

    /// Source path within the output (e.g., "/content" for file derivations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_subpath: Option<String>,
}

/// The type of derivation (for internal classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DerivationType {
    /// A file derivation (content or source)
    File,
    /// An environment variable derivation
    Env,
    /// A package derivation
    Package,
    /// A custom/user-defined derivation
    Custom,
}

/// Metadata about a derivation's origin and purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationMeta {
    /// Type of derivation
    pub derivation_type: DerivationType,

    /// Original file path (for file derivations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_path: Option<PathBuf>,

    /// Environment variable name (for env derivations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_name: Option<String>,

    /// Package should be added to PATH
    #[serde(default)]
    pub add_to_path: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derivation_spec_hash() {
        let spec = DerivationSpec {
            name: "test".to_string(),
            version: Some("1.0.0".to_string()),
            inputs: BTreeMap::new(),
            build_hash: "build123".to_string(),
            outputs: vec!["out".to_string()],
            system: System {
                platform: "x86_64-linux".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                hostname: "test".to_string(),
                username: "user".to_string(),
            },
        };

        let hash = spec.compute_hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex

        // Same spec should produce same hash
        let hash2 = spec.compute_hash();
        assert_eq!(hash, hash2);

        // Different spec should produce different hash
        let mut spec2 = spec.clone();
        spec2.version = Some("2.0.0".to_string());
        let hash3 = spec2.compute_hash();
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_derivation_new() {
        let spec = DerivationSpec {
            name: "ripgrep".to_string(),
            version: Some("15.1.0".to_string()),
            inputs: BTreeMap::new(),
            build_hash: "xyz".to_string(),
            outputs: vec!["out".to_string()],
            system: System {
                platform: "aarch64-darwin".to_string(),
                os: "darwin".to_string(),
                arch: "aarch64".to_string(),
                hostname: "mac".to_string(),
                username: "ian".to_string(),
            },
        };

        let drv = Derivation::new(spec);
        assert_eq!(drv.name(), "ripgrep");
        assert_eq!(drv.version(), Some("15.1.0"));
        assert!(!drv.realized);
        assert!(drv.output_paths.is_empty());
    }

    #[test]
    fn test_input_value_serialization() {
        let mut table = BTreeMap::new();
        table.insert(
            "url".to_string(),
            InputValue::String("https://example.com".to_string()),
        );
        table.insert("count".to_string(), InputValue::Number(42.0));
        table.insert("enabled".to_string(), InputValue::Bool(true));

        let json = serde_json::to_string(&table).unwrap();
        let parsed: BTreeMap<String, InputValue> = serde_json::from_str(&json).unwrap();
        assert_eq!(table, parsed);
    }
}
