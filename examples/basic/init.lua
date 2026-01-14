--- Basic syslua configuration example
--- Entry point must return a table with `inputs` and `setup` fields
return {
  inputs = {
    syslua = 'github:spirit-led-software/syslua/master', -- includes pkgs, lib, modules
    dotfiles = 'github:ianpascoe/dotfiles/master', -- includes dotfiles, not a lua module
  },
  setup = function(inputs)
    local syslua = require('syslua')
    local file = syslua.modules.file
    local path = sys.path

    file.setup({
      target = path.resolve(path.join(sys.getenv('HOME'), '.config', 'starship.toml')),
      source = path.resolve(path.join(inputs.dotfiles.path, 'config', 'starship.toml')),
    })
    file.setup({
      target = path.resolve(path.join(sys.getenv('HOME'), '.ssh')),
      source = path.resolve(path.join(sys.dir, '..', 'dotfiles', '.ssh')),
      mutable = true,
    })
  end,
}
