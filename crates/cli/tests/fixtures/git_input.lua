--- Git input resolution test.
--- This test is slow and requires network access, so it's marked #[ignore].

return {
  inputs = {
    -- Use a small, stable repository for testing
    example_repo = {
      git = 'https://github.com/octocat/Hello-World.git',
      ref = 'master',
    },
  },
  setup = function(inputs)
    -- Just verify the input was resolved
    -- The actual file contents don't matter for this test
    sys.build({
      id = 'git-input-test',
      inputs = { repo = inputs.example_repo },
      create = function(build_inputs, ctx)
        -- The repo path should exist
        return { repo_path = build_inputs.repo }
      end,
    })
  end,
}
