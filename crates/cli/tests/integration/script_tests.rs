//! Script method integration tests.

use predicates::prelude::*;

use super::common::TestEnv;

/// Test that ctx:script() with an invalid format produces a helpful error.
#[test]
fn script_invalid_format_errors() {
  let env = TestEnv::empty();

  env.write_file(
    "init.lua",
    r#"
return {
    inputs = {},
    setup = function()
        sys.register_build_ctx_method('script', function(ctx, format, content, opts)
            if format ~= 'shell' and format ~= 'bash' and format ~= 'powershell' and format ~= 'cmd' then
                error("script() format must be 'shell', 'bash', 'powershell', or 'cmd', got: " .. tostring(format))
            end
            return {}
        end)

        sys.build({
            id = 'test-invalid-format',
            create = function(_inputs, ctx)
                ctx:script('invalid', [[echo "bad"]])
                return { out = ctx.out }
            end,
        })
    end,
}
"#,
  );

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .failure()
    .stderr(predicate::str::contains("format must be"));
}
