use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use mlua::prelude::*;

use crate::lua::{loaders, runtime};
use crate::manifest::Manifest;

pub fn extract_inputs(entrypoint_path: &str) -> LuaResult<HashMap<String, String>> {
  let manifest = Rc::new(RefCell::new(Manifest::default()));
  let lua = runtime::create_runtime(manifest)?;

  let path = Path::new(entrypoint_path);
  let result = loaders::load_file_with_dir(&lua, path)?;
  let result_table = result
    .as_table()
    .ok_or_else(|| LuaError::external("entrypoint must return a table"))?;

  let inputs_table: LuaTable = result_table.get("inputs")?;

  let mut inputs = HashMap::new();
  for pair in inputs_table.pairs::<String, String>() {
    let (key, value) = pair?;
    inputs.insert(key, value);
  }

  Ok(inputs)
}
