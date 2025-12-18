--- Example usage of syslua.lib.fetch_url
--- This fixture demonstrates how to use fetch_url to download a file.

local syslua = require('syslua')

-- Fetch a file from a URL with SHA256 verification
local fetched = syslua.lib.fetch_url({
  url = 'https://example.com/file.tar.gz',
  sha256 = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
})

-- The fetched result can be used as an input to other builds
sys.build({
  id = 'uses-fetched-file',
  inputs = { fetched = fetched },
  create = function(inputs, ctx)
    -- Use inputs.fetched.outputs.out to access the downloaded file
    return { out = ctx.out }
  end,
})
