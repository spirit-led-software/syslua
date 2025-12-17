# Plan: Release Workflow

## Goal

Create an automated GitHub Actions release pipeline that builds binaries for all platforms, generates changelogs from conventional commits, and publishes GitHub Releases.

## Overview

The release system consists of two workflows:

1. **Release PR Workflow** - Automates version bumping and changelog generation
2. **Release Build Workflow** - Builds and publishes binaries when a version tag is pushed

## Conventional Commits

All commits should follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

| Prefix | Description | Changelog Section |
|--------|-------------|-------------------|
| `feat:` | New feature | Features |
| `fix:` | Bug fix | Bug Fixes |
| `docs:` | Documentation | Documentation |
| `refactor:` | Code refactoring | Refactoring |
| `perf:` | Performance improvement | Performance |
| `test:` | Test changes | Testing |
| `chore:` | Maintenance | Other |
| `BREAKING CHANGE:` | Breaking change | Breaking Changes |

## Changelog Generation

Use [git-cliff](https://github.com/orhun/git-cliff) to generate changelogs from commit history.

### Configuration (`cliff.toml`)

```toml
[changelog]
header = "# Changelog\n\nAll notable changes to this project will be documented in this file.\n"
body = """
{% for group, commits in commits | group_by(attribute="group") %}
## {{ group | upper_first }}
{% for commit in commits %}
- {{ commit.message | upper_first }} ([{{ commit.id | truncate(length=7, end="") }}](https://github.com/spirit-led-software/syslua/commit/{{ commit.id }}))
{%- endfor %}
{% endfor %}
"""
trim = true

[git]
conventional_commits = true
filter_unconventional = true
commit_parsers = [
  { message = "^feat", group = "Features" },
  { message = "^fix", group = "Bug Fixes" },
  { message = "^doc", group = "Documentation" },
  { message = "^perf", group = "Performance" },
  { message = "^refactor", group = "Refactoring" },
  { message = "^test", group = "Testing" },
  { message = "^chore", group = "Other" },
]
filter_commits = false
tag_pattern = "v[0-9]*"
```

## Target Platforms

| OS | Architecture | Target Triple | Build Method |
|----|--------------|---------------|--------------|
| Linux | x86_64 | `x86_64-unknown-linux-musl` | Native |
| Linux | aarch64 | `aarch64-unknown-linux-musl` | Cross |
| macOS | x86_64 | `x86_64-apple-darwin` | Native |
| macOS | aarch64 | `aarch64-apple-darwin` | Native |
| Windows | x86_64 | `x86_64-pc-windows-msvc` | Native |

**Note:** Linux uses musl for fully static binaries that work on any Linux distribution.

## Artifact Naming

```
sys-<version>-<target>.<ext>
```

Examples:
- `sys-v0.1.0-x86_64-unknown-linux-musl.tar.gz`
- `sys-v0.1.0-aarch64-apple-darwin.tar.gz`
- `sys-v0.1.0-x86_64-pc-windows-msvc.zip`

### Archive Contents

Each archive contains:
```
sys-v0.1.0-x86_64-unknown-linux-musl/
├── sys          # The binary
├── README.md
└── LICENSE
```

## Workflow 1: Release PR

**Trigger:** Manual dispatch or schedule

**Purpose:** Creates a PR that bumps version in `Cargo.toml` and updates `CHANGELOG.md`

```yaml
name: Release PR

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version to release (e.g., 0.1.0)'
        required: true

jobs:
  release-pr:
    runs-on: ubuntu-latest
    steps:
      - Checkout
      - Install git-cliff
      - Update version in Cargo.toml files
      - Generate changelog with git-cliff
      - Create PR with changes
```

## Workflow 2: Release Build

**Trigger:** Push of tags matching `v*`

**Purpose:** Build binaries and create GitHub Release

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
          - target: aarch64-unknown-linux-musl
            os: ubuntu-latest
            cross: true
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest
    steps:
      - Checkout
      - Install Rust + target
      - Build release binary
      - Rename binary to 'sys'
      - Create archive with README and LICENSE
      - Upload artifact

  release:
    needs: build
    steps:
      - Download all artifacts
      - Generate SHA256 checksums
      - Extract changelog for this version
      - Create GitHub Release with artifacts
```

## Release Process

1. **Prepare Release:**
   - Go to Actions → Release PR → Run workflow
   - Enter version number (e.g., `0.2.0`)
   - Workflow creates PR with version bump and changelog

2. **Review & Merge:**
   - Review the generated changelog
   - Make any manual adjustments
   - Merge the PR

3. **Tag & Release:**
   - After merge, tag the release: `git tag v0.2.0 && git push --tags`
   - Release workflow automatically builds and publishes

## Files to Create

| Path | Purpose |
|------|---------|
| `.github/workflows/release.yml` | Build and publish releases |
| `.github/workflows/release-pr.yml` | Create release PRs |
| `cliff.toml` | git-cliff configuration |
| `CHANGELOG.md` | Generated changelog |

## Files to Modify

| Path | Changes |
|------|---------|
| `crates/cli/Cargo.toml` | Rename binary from `syslua-cli` to `sys` |

## Success Criteria

1. `git tag v0.1.0 && git push --tags` triggers automated release
2. Binaries are built for all 5 platform/arch combinations
3. Archives include binary, README.md, and LICENSE
4. SHA256 checksums are published
5. GitHub Release is created with all artifacts
6. Changelog is included in release notes
7. Release PR workflow correctly bumps versions and generates changelog

## Future Enhancements

- [ ] Homebrew formula generation
- [ ] AUR package for Arch Linux
- [ ] Scoop manifest for Windows
- [ ] Shell installer script (`curl | sh`)
