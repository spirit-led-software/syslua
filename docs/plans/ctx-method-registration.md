# Plan: Custom Context Method Registration

## Goal

Expose `sys.register_ctx_method()` and `sys.unregister_ctx_method()` for Lua libraries to extend BuildCtx and BindCtx.

## Problem

The architecture describes custom context methods, but they're not exposed on the `sys` global table.

## Architecture Reference

- [04-lua-api.md](../architecture/04-lua-api.md):127-159 - Custom ActionCtx methods

## Approach

1. Add `register_ctx_method` and `unregister_ctx_method` to the `sys` table
2. Store registered methods in Lua registry
3. Apply registered methods when creating BuildCtx/BindCtx
4. Prevent overriding built-in methods

## Lua API

```lua
-- Register a cross-platform mkdir helper
sys.register_ctx_method("mkdir", function(ctx, path)
    if sys.os == "windows" then
        return ctx:exec({ bin = "cmd.exe", args = { "/c", "mkdir", path } })
    else
        return ctx:exec({ bin = "/bin/mkdir", args = { "-p", path } })
    end
end)

-- Now available on any context:
sys.build({
    name = "my-tool",
    apply = function(inputs, ctx)
        ctx:mkdir(ctx.out .. "/bin")  -- Uses registered method
        return { out = ctx.out }
    end,
})

-- Unregister if needed
sys.unregister_ctx_method("mkdir")
```

## Implementation

```rust
// In lua/globals.rs
fn register_ctx_method(lua: &Lua, name: String, func: Function) -> LuaResult<()> {
    // Prevent overriding built-ins
    let builtins = ["exec", "fetch_url", "write_file", "out"];
    if builtins.contains(&name.as_str()) {
        return Err(LuaError::RuntimeError(
            format!("Cannot override built-in method: {}", name)
        ));
    }
    
    // Store in Lua registry
    let methods: Table = lua.named_registry_value("syslua_ctx_methods")?;
    methods.set(name, func)?;
    Ok(())
}
```

## Files to Modify

| Path | Changes |
|------|---------|
| `crates/lib/src/lua/globals.rs` | Add register/unregister functions |
| `crates/lib/src/action/lua.rs` | Apply registered methods to context |

## Success Criteria

1. `sys.register_ctx_method()` registers custom methods
2. Registered methods available on BuildCtx and BindCtx
3. Built-in methods cannot be overridden
4. `sys.unregister_ctx_method()` removes methods
5. Clear error for unknown method calls

## Open Questions

- [ ] Should methods be scoped (per-file or global)?
- [ ] How to handle method name conflicts between libraries?
- [ ] Should there be a way to list registered methods?
