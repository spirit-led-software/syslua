//! Tests for syslua.groups module.

use mlua::prelude::*;
use syslua_lib::bind::BindInputsDef;

use super::common::create_test_runtime;

fn get_input_table(inputs: &Option<BindInputsDef>) -> &std::collections::BTreeMap<String, BindInputsDef> {
  match inputs.as_ref().expect("should have inputs") {
    BindInputsDef::Table(t) => t,
    _ => panic!("inputs should be a table"),
  }
}

#[test]
fn module_loads_without_error() -> LuaResult<()> {
  let (lua, _) = create_test_runtime()?;

  lua.load("local groups = require('syslua.groups')").exec()?;

  Ok(())
}

#[test]
fn setup_requires_groups() -> LuaResult<()> {
  let (lua, _) = create_test_runtime()?;

  let result = lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({})
      "#,
    )
    .exec();

  assert!(result.is_err());
  let err_msg = result.unwrap_err().to_string();
  assert!(
    err_msg.contains("at least one group"),
    "Expected error about empty groups, got: {}",
    err_msg
  );
  Ok(())
}

#[test]
fn setup_creates_bind_for_each_group() -> LuaResult<()> {
  let (lua, manifest) = create_test_runtime()?;

  lua.load("sys.is_elevated = true").exec()?;

  lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({
          developers = { description = 'Dev team' },
          admins = { description = 'Admin team' },
        })
      "#,
    )
    .exec()?;

  let m = manifest.borrow();
  assert_eq!(m.bindings.len(), 2, "should create one bind per group");
  Ok(())
}

#[test]
fn bind_id_uses_prefix() -> LuaResult<()> {
  let (lua, manifest) = create_test_runtime()?;

  lua.load("sys.is_elevated = true").exec()?;

  lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({
          mygroup = { description = 'My Group' },
        })
      "#,
    )
    .exec()?;

  let m = manifest.borrow();
  let bind = m.bindings.values().next().expect("should have a binding");
  assert_eq!(
    bind.id,
    Some("__syslua_group_mygroup".to_string()),
    "bind id should use __syslua_group_ prefix"
  );
  Ok(())
}

#[test]
fn setup_with_gid_option() -> LuaResult<()> {
  let (lua, manifest) = create_test_runtime()?;

  lua.load("sys.is_elevated = true").exec()?;

  lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({
          testgroup = { gid = 2001 },
        })
      "#,
    )
    .exec()?;

  let m = manifest.borrow();
  let bind = m.bindings.values().next().expect("should have a binding");
  let inputs = get_input_table(&bind.inputs);
  let gid = inputs.get("gid").expect("should have gid input");
  assert!(
    matches!(gid, BindInputsDef::Number(n) if *n == 2001.0),
    "gid should be 2001"
  );
  Ok(())
}

#[test]
fn setup_with_system_option() -> LuaResult<()> {
  let (lua, manifest) = create_test_runtime()?;

  lua.load("sys.is_elevated = true").exec()?;

  lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({
          sysgroup = { system = true },
        })
      "#,
    )
    .exec()?;

  let m = manifest.borrow();
  let bind = m.bindings.values().next().expect("should have a binding");
  let inputs = get_input_table(&bind.inputs);
  let system = inputs.get("system").expect("should have system input");
  assert!(matches!(system, BindInputsDef::Boolean(true)), "system should be true");
  Ok(())
}

#[test]
fn defaults_are_applied() -> LuaResult<()> {
  let (lua, manifest) = create_test_runtime()?;

  lua.load("sys.is_elevated = true").exec()?;

  lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({
          minimalgroup = {},
        })
      "#,
    )
    .exec()?;

  let m = manifest.borrow();
  let bind = m.bindings.values().next().expect("should have a binding");
  let inputs = get_input_table(&bind.inputs);

  let desc = inputs.get("description").expect("should have description");
  assert!(
    matches!(desc, BindInputsDef::String(s) if s.is_empty()),
    "description should default to empty"
  );

  let system = inputs.get("system").expect("should have system");
  assert!(
    matches!(system, BindInputsDef::Boolean(false)),
    "system should default to false"
  );

  Ok(())
}

#[test]
fn bind_has_create_update_destroy_actions() -> LuaResult<()> {
  let (lua, manifest) = create_test_runtime()?;

  lua.load("sys.is_elevated = true").exec()?;

  lua
    .load(
      r#"
        local groups = require('syslua.groups')
        groups.setup({
          actiongroup = { description = 'Test' },
        })
      "#,
    )
    .exec()?;

  let m = manifest.borrow();
  let bind = m.bindings.values().next().expect("should have a binding");

  assert!(!bind.create_actions.is_empty(), "should have create actions");
  assert!(!bind.destroy_actions.is_empty(), "should have destroy actions");
  assert!(bind.update_actions.is_some(), "should have update actions");
  Ok(())
}
