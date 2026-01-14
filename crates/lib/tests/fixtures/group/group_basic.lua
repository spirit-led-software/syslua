local groups = require('syslua.groups')

return {
  inputs = {},
  setup = function(_)
    groups.setup({
      testgroup = {
        description = 'Test Group',
        gid = 2001,
      },
      sysgroup = {
        description = 'System Test Group',
        system = true,
      },
    })
  end,
}
