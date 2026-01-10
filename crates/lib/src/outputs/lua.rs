use std::collections::BTreeMap;

use mlua::prelude::*;
use serde_json::Value as JsonValue;

/// Convert a Lua value to a serde_json::Value.
fn lua_value_to_json(value: LuaValue) -> LuaResult<JsonValue> {
  match value {
    LuaValue::Nil => Ok(JsonValue::Null),
    LuaValue::Boolean(b) => Ok(JsonValue::Bool(b)),
    LuaValue::Integer(i) => Ok(JsonValue::Number(i.into())),
    LuaValue::Number(n) => {
      // Handle potential NaN/Infinity which aren't valid JSON
      if n.is_finite() {
        Ok(serde_json::Number::from_f64(n).map_or(JsonValue::Null, JsonValue::Number))
      } else {
        Err(LuaError::external(
          "output numbers must be finite (not NaN or Infinity)",
        ))
      }
    }
    LuaValue::String(s) => Ok(JsonValue::String(s.to_str()?.to_string())),
    LuaValue::Table(t) => {
      // Check if this is an array (sequential integer keys starting at 1) or object
      let mut is_array = true;
      let mut max_index = 0;
      for pair in t.clone().pairs::<LuaValue, LuaValue>() {
        let (k, _) = pair?;
        match k {
          LuaValue::Integer(i) if i > 0 => {
            max_index = max_index.max(i as usize);
          }
          _ => {
            is_array = false;
            break;
          }
        }
      }

      if is_array && max_index > 0 {
        // Convert as array
        let mut arr = Vec::with_capacity(max_index);
        for i in 1..=max_index {
          let v: LuaValue = t.get(i)?;
          arr.push(lua_value_to_json(v)?);
        }
        Ok(JsonValue::Array(arr))
      } else {
        // Convert as object
        let mut map = serde_json::Map::new();
        for pair in t.pairs::<String, LuaValue>() {
          let (k, v) = pair?;
          map.insert(k, lua_value_to_json(v)?);
        }
        Ok(JsonValue::Object(map))
      }
    }
    LuaValue::Function(_) => Err(LuaError::external("output values cannot be functions")),
    LuaValue::Thread(_) => Err(LuaError::external("output values cannot be threads")),
    LuaValue::UserData(_) => Err(LuaError::external("output values cannot be userdata")),
    LuaValue::LightUserData(_) => Err(LuaError::external("output values cannot be light userdata")),
    LuaValue::Error(e) => Err(LuaError::external(format!("output values cannot be errors: {}", e))),
    _ => Err(LuaError::external("unsupported output value type")),
  }
}

/// Convert a JSON value to a Lua value.
fn json_to_lua_value(lua: &Lua, value: &JsonValue) -> LuaResult<LuaValue> {
  match value {
    JsonValue::Null => Ok(LuaValue::Nil),
    JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
    JsonValue::Number(n) => {
      if let Some(i) = n.as_i64() {
        Ok(LuaValue::Integer(i))
      } else if let Some(f) = n.as_f64() {
        Ok(LuaValue::Number(f))
      } else {
        Err(LuaError::external("invalid number in output"))
      }
    }
    JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
    JsonValue::Array(arr) => {
      let table = lua.create_table()?;
      for (i, v) in arr.iter().enumerate() {
        table.set(i + 1, json_to_lua_value(lua, v)?)?;
      }
      Ok(LuaValue::Table(table))
    }
    JsonValue::Object(obj) => {
      let table = lua.create_table()?;
      for (k, v) in obj {
        table.set(k.as_str(), json_to_lua_value(lua, v)?)?;
      }
      Ok(LuaValue::Table(table))
    }
  }
}

/// Convert a Lua table of outputs to a BTreeMap with JSON values.
///
/// Outputs can be any JSON-serializable type: strings, numbers, booleans, null, arrays, or objects.
pub fn parse_outputs(table: LuaTable) -> LuaResult<BTreeMap<String, JsonValue>> {
  let mut outputs = BTreeMap::new();
  for pair in table.pairs::<String, LuaValue>() {
    let (k, v) = pair?;
    outputs.insert(k, lua_value_to_json(v)?);
  }
  Ok(outputs)
}

/// Convert a BTreeMap of JSON outputs back to a Lua table.
pub fn outputs_to_lua_table(lua: &Lua, outputs: &BTreeMap<String, JsonValue>) -> LuaResult<LuaTable> {
  let table = lua.create_table()?;
  for (k, v) in outputs {
    table.set(k.as_str(), json_to_lua_value(lua, v)?)?;
  }
  Ok(table)
}
