use mlua::prelude::*;

use crate::action::{ActionCtx, actions::cmd::parse_cmd_opts};

impl LuaUserData for ActionCtx {
  fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
    fields.add_field_method_get("out", |_, this| Ok(this.out().to_string()));
  }

  fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
    methods.add_method_mut("fetch_url", |_, this, (url, sha256): (String, String)| {
      Ok(this.fetch_url(&url, &sha256))
    });

    methods.add_method_mut("cmd", |_, this, opts: LuaValue| {
      let cmd_opts = parse_cmd_opts(opts)?;
      Ok(this.cmd(cmd_opts))
    });
  }
}
