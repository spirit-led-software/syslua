use std::collections::BTreeMap;

use mlua::Function;
use serde::{Deserialize, Serialize};

use crate::{bind::BindHash, build::BuildHash};

/// The inputs specification, either static or dynamic (function).
pub enum InputsSpec {
  Static(InputsRef),
  Dynamic(Function),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputsRef {
  String(String),
  Number(f64),
  Boolean(bool),
  Table(BTreeMap<String, InputsRef>),
  Array(Vec<InputsRef>),
  Build(BuildHash),
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
