//! Tests for syslua.modules.* functions.

use mlua::prelude::*;

use super::common::create_test_runtime;

mod file_module {
  use super::*;

  #[test]
  fn requires_target() -> LuaResult<()> {
    let (lua, _) = create_test_runtime()?;

    let result = lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.modules.file.setup({ content = 'hello' })
        "#,
      )
      .exec();

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
      err_msg.contains("requires a 'target'"),
      "Expected error about missing target, got: {}",
      err_msg
    );
    Ok(())
  }

  #[test]
  fn requires_source_or_content() -> LuaResult<()> {
    let (lua, _) = create_test_runtime()?;

    let result = lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.modules.file.setup({ target = '/tmp/test.txt' })
        "#,
      )
      .exec();

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
      err_msg.contains("source' or 'content'"),
      "Expected error about missing source/content, got: {}",
      err_msg
    );
    Ok(())
  }

  #[test]
  fn mutable_creates_bind_only() -> LuaResult<()> {
    let (lua, manifest) = create_test_runtime()?;

    lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.modules.file.setup({
                target = '/tmp/test.txt',
                content = 'hello world',
                mutable = true,
            })
        "#,
      )
      .exec()?;

    let m = manifest.borrow();
    assert_eq!(m.builds.len(), 0, "mutable file should not create a build");
    assert_eq!(m.bindings.len(), 1, "mutable file should create a bind");
    Ok(())
  }

  #[test]
  fn immutable_creates_build_and_bind() -> LuaResult<()> {
    let (lua, manifest) = create_test_runtime()?;

    lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.modules.file.setup({
                target = '/tmp/test.txt',
                content = 'hello world',
                mutable = false,
            })
        "#,
      )
      .exec()?;

    let m = manifest.borrow();
    assert_eq!(m.builds.len(), 1, "immutable file should create a build");
    assert_eq!(m.bindings.len(), 1, "immutable file should create a bind");
    Ok(())
  }

  #[test]
  fn default_is_immutable() -> LuaResult<()> {
    let (lua, manifest) = create_test_runtime()?;

    lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.modules.file.setup({
                target = '/tmp/test.txt',
                content = 'hello world',
            })
        "#,
      )
      .exec()?;

    let m = manifest.borrow();
    assert_eq!(m.builds.len(), 1, "default file should be immutable (create a build)");
    assert_eq!(m.bindings.len(), 1, "default file should create a bind");
    Ok(())
  }
}
