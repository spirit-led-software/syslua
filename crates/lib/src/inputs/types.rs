//! Input types for syslua.
//!
//! This module defines types for parameterizing builds and bindings with inputs.
//! Inputs can be static values or dynamically computed from Lua functions.
//!
//! # Two Representations
//!
//! - [`InputsSpec`]: The Lua-side representation, which may contain a function
//! - [`InputsRef`]: The serializable representation with all values resolved
//!
//! # Reference Types
//!
//! When a build or binding depends on another, the dependency is stored as
//! [`InputsRef::Build`] or [`InputsRef::Bind`] containing just the hash.
//! This keeps the manifest compact and enables efficient serialization.

use std::collections::BTreeMap;

use mlua::Function;
use serde::{Deserialize, Serialize};

use crate::{bind::BindHash, build::BuildHash};

/// The inputs specification, either static or dynamic (function).
///
/// This is the Lua-side representation of inputs before evaluation.
///
/// # Variants
///
/// - [`Static`](InputsSpec::Static): A pre-computed value
/// - [`Dynamic`](InputsSpec::Dynamic): A Lua function that computes inputs at evaluation time
///
/// Dynamic inputs enable lazy evaluation and can access runtime context.
pub enum InputsSpec {
  /// Pre-computed static input values.
  Static(InputsRef),
  /// A Lua function that returns input values when called.
  Dynamic(Function),
}

/// A resolved, serializable input value.
///
/// This is the manifest-side representation of inputs. All values are fully
/// resolved and can be serialized to JSON.
///
/// # Primitive Types
///
/// - [`String`](InputsRef::String): Text values
/// - [`Number`](InputsRef::Number): Floating-point numbers
/// - [`Boolean`](InputsRef::Boolean): True/false values
///
/// # Collection Types
///
/// - [`Table`](InputsRef::Table): Key-value maps (Lua tables with string keys)
/// - [`Array`](InputsRef::Array): Ordered sequences (Lua tables with numeric keys)
///
/// # Reference Types
///
/// - [`Build`](InputsRef::Build): Reference to a build by its hash
/// - [`Bind`](InputsRef::Bind): Reference to a binding by its hash
///
/// # Reference Storage
///
/// When storing references to builds or bindings, only the hash is stored
/// (not the full definition). This:
/// - Keeps the manifest compact
/// - Avoids circular reference issues during serialization
/// - Enables efficient dependency tracking
///
/// # Example
///
/// ```json
/// {
///   "Table": {
///     "name": { "String": "myapp" },
///     "debug": { "Boolean": false },
///     "rust": { "Build": "a1b2c3d4e5f6789012ab" }
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputsRef {
  /// A string value.
  String(String),
  /// A numeric value (f64 to match Lua's number type).
  Number(f64),
  /// A boolean value.
  Boolean(bool),
  /// A table (map) with string keys.
  Table(BTreeMap<String, InputsRef>),
  /// An array (sequence) of values.
  Array(Vec<InputsRef>),
  /// A reference to a build, stored as its [`BuildHash`].
  Build(BuildHash),
  /// A reference to a binding, stored as its [`BindHash`].
  Bind(BindHash),
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::build::BuildHash;

  #[test]
  fn complex_nested_structure_roundtrip() {
    // Simulates a realistic input structure like:
    // {
    //   src = { url = "...", sha256 = "..." },
    //   features = ["a", "b"],
    //   debug = false,
    //   rust = <build hash ref>,
    // }
    let mut src = BTreeMap::new();
    src.insert(
      "url".to_string(),
      InputsRef::String("https://example.com/pkg.tar.gz".to_string()),
    );
    src.insert("sha256".to_string(), InputsRef::String("abc123".to_string()));

    let features = InputsRef::Array(vec![
      InputsRef::String("feature_a".to_string()),
      InputsRef::String("feature_b".to_string()),
    ]);

    // BuildHash is just the hash - no name/version/outputs stored in InputsRef
    let rust_hash = BuildHash("abc123def456789012345678901234567890123456789012345678901234".to_string());

    let mut inputs = BTreeMap::new();
    inputs.insert("src".to_string(), InputsRef::Table(src));
    inputs.insert("features".to_string(), features);
    inputs.insert("debug".to_string(), InputsRef::Boolean(false));
    inputs.insert("rust".to_string(), InputsRef::Build(rust_hash));

    let value = InputsRef::Table(inputs);

    // Verify serialization roundtrip preserves all nested structure
    let json = serde_json::to_string(&value).unwrap();
    let deserialized: InputsRef = serde_json::from_str(&json).unwrap();
    assert_eq!(value, deserialized);
  }
}
