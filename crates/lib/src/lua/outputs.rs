use std::collections::BTreeMap;

use mlua::prelude::*;

/// Convert a Lua table of outputs to a BTreeMap.
pub fn parse_outputs(table: LuaTable) -> LuaResult<BTreeMap<String, String>> {
  let mut outputs = BTreeMap::new();
  for pair in table.pairs::<String, String>() {
    let (k, v) = pair?;
    outputs.insert(k, v);
  }
  Ok(outputs)
}
