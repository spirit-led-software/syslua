use mlua::prelude::*;

use crate::action::actions::exec::parse_exec_opts;
use crate::action::{ActionCtx, CTX_METHODS_REGISTRY_KEY};

impl LuaUserData for ActionCtx {
  fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
    fields.add_field_method_get("out", |_, this| Ok(this.out().to_string()));
  }

  fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
    methods.add_method_mut("fetch_url", |_, this, (url, sha256): (String, String)| {
      Ok(this.fetch_url(&url, &sha256))
    });

    methods.add_method_mut("write_file", |_, this, (path, contents): (String, String)| {
      Ok(this.write_file(&path, &contents))
    });

    methods.add_method_mut("exec", |_, this, (opts, args): (LuaValue, Option<LuaValue>)| {
      let cmd_opts = parse_exec_opts(opts, args)?;
      Ok(this.exec(cmd_opts))
    });

    // Fallback for custom registered methods
    methods.add_meta_method(mlua::MetaMethod::Index, |lua, _this, key: String| {
      // Look up the method in the registry
      let registry: LuaTable = lua.named_registry_value(CTX_METHODS_REGISTRY_KEY)?;
      let func: LuaValue = registry.get(key.as_str())?;

      match func {
        LuaValue::Function(_) => Ok(func),
        LuaValue::Nil => Err(LuaError::external(format!(
          "unknown ctx method '{}'. Use sys.register_ctx_method to add custom methods.",
          key
        ))),
        _ => Err(LuaError::external(format!("ctx method '{}' is not a function", key))),
      }
    });
  }
}

#[cfg(test)]
mod tests {
  use std::cell::RefCell;
  use std::rc::Rc;

  use super::*;
  use crate::lua::globals::register_globals;
  use crate::manifest::Manifest;

  fn create_test_lua_with_ctx() -> LuaResult<(Lua, ActionCtx)> {
    let lua = Lua::new();
    let manifest = Rc::new(RefCell::new(Manifest::default()));
    register_globals(&lua, manifest)?;

    let ctx = ActionCtx::new();
    Ok((lua, ctx))
  }

  #[test]
  fn registered_method_can_be_called_on_ctx() -> LuaResult<()> {
    let (lua, ctx) = create_test_lua_with_ctx()?;

    // Register a custom method
    lua
      .load(
        r#"
      sys.register_ctx_method("greet", function(ctx, name)
        return "Hello, " .. name .. "!"
      end)
      "#,
      )
      .exec()?;

    // Set the ctx as a global
    lua.globals().set("ctx", ctx)?;

    // Call the custom method
    let result: String = lua.load(r#"return ctx:greet("World")"#).eval()?;
    assert_eq!(result, "Hello, World!");
    Ok(())
  }

  #[test]
  fn registered_method_receives_ctx_as_first_arg() -> LuaResult<()> {
    let (lua, ctx) = create_test_lua_with_ctx()?;

    // Register a method that uses ctx.out
    lua
      .load(
        r#"
      sys.register_ctx_method("get_out_path", function(ctx)
        return ctx.out
      end)
      "#,
      )
      .exec()?;

    lua.globals().set("ctx", ctx)?;

    let result: String = lua.load(r#"return ctx:get_out_path()"#).eval()?;
    // ctx.out returns the placeholder "$${out}"
    assert_eq!(result, "$${out}");
    Ok(())
  }

  #[test]
  fn unknown_method_gives_helpful_error() -> LuaResult<()> {
    let (lua, ctx) = create_test_lua_with_ctx()?;
    lua.globals().set("ctx", ctx)?;

    let result = lua.load(r#"return ctx:nonexistent_method()"#).eval::<String>();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown ctx method 'nonexistent_method'"));
    assert!(err.contains("sys.register_ctx_method"));
    Ok(())
  }

  #[test]
  fn builtin_methods_still_work() -> LuaResult<()> {
    let (lua, ctx) = create_test_lua_with_ctx()?;
    lua.globals().set("ctx", ctx)?;

    // out field should work
    let out: String = lua.load(r#"return ctx.out"#).eval()?;
    assert_eq!(out, "$${out}");

    Ok(())
  }
}
