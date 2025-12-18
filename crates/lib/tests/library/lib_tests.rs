//! Tests for syslua.lib.* functions.

use mlua::prelude::*;

use super::common::create_test_runtime;

mod fetch_url {
  use super::*;

  #[test]
  fn requires_url() -> LuaResult<()> {
    let (lua, _) = create_test_runtime()?;

    let result = lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.lib.fetch_url({ sha256 = 'abc123' })
        "#,
      )
      .exec();

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
      err_msg.contains("requires a 'url'"),
      "Expected error about missing url, got: {}",
      err_msg
    );
    Ok(())
  }

  #[test]
  fn requires_sha256() -> LuaResult<()> {
    let (lua, _) = create_test_runtime()?;

    let result = lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.lib.fetch_url({ url = 'https://example.com/file.tar.gz' })
        "#,
      )
      .exec();

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
      err_msg.contains("requires a 'sha256'"),
      "Expected error about missing sha256, got: {}",
      err_msg
    );
    Ok(())
  }

  #[test]
  fn creates_build_in_manifest() -> LuaResult<()> {
    let (lua, manifest) = create_test_runtime()?;

    lua
      .load(
        r#"
            local syslua = require('syslua')
            syslua.lib.fetch_url({
                url = 'https://example.com/file.tar.gz',
                sha256 = 'abc123def456',
            })
        "#,
      )
      .exec()?;

    let m = manifest.borrow();
    assert_eq!(m.builds.len(), 1, "fetch_url should create exactly one build");
    Ok(())
  }
}
