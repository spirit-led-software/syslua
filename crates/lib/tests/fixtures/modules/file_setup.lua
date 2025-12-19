--- Example usage of syslua.modules.file.setup
--- This fixture demonstrates how to use the file module.

local syslua = require('syslua')

-- Create an immutable file from content (default behavior)
syslua.modules.file.setup({
  target = '/home/user/.config/app/config.txt',
  content = 'key=value',
})

-- Create a mutable file from content
syslua.modules.file.setup({
  target = '/home/user/.local/share/app/data.txt',
  content = 'mutable data',
  mutable = true,
})
