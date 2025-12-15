//! Input source URL parsing.
//!
//! This module parses input URL strings into typed [`InputSource`] values.
//!
//! # Supported Formats
//!
//! - `git:https://github.com/org/repo.git` - Git over HTTPS (HEAD)
//! - `git:https://github.com/org/repo.git#v1.0.0` - Git with specific ref (tag/branch/commit)
//! - `git:git@github.com:org/repo.git` - Git over SSH
//! - `git:git@github.com:org/repo.git#main` - Git over SSH with specific ref
//! - `path:~/code/foo` - Absolute path with tilde expansion
//! - `path:./relative` - Relative path (resolved against config dir)

use std::path::PathBuf;

use thiserror::Error;

/// A parsed input source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSource {
  /// A git repository to clone/fetch.
  Git {
    /// The git URL (without the `git:` prefix and `#ref` suffix).
    url: String,
    /// Optional ref to checkout (branch, tag, or commit hash).
    /// If None, uses HEAD (default branch).
    rev: Option<String>,
  },
  /// A local filesystem path.
  Path {
    /// The path string (may contain `~` or be relative).
    path: PathBuf,
  },
}

/// Errors that can occur when parsing an input URL.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
  /// The URL scheme (prefix before `:`) is not recognized.
  #[error("unknown input scheme '{0}': expected 'git:' or 'path:'")]
  UnknownScheme(String),

  /// The URL is missing content after the scheme prefix.
  #[error("missing URL after 'git:' prefix")]
  MissingGitUrl,

  /// The path is missing after the `path:` prefix.
  #[error("missing path after 'path:' prefix")]
  MissingPath,

  /// The ref after `#` is empty.
  #[error("empty ref after '#' in git URL")]
  EmptyGitRef,
}

/// Parse an input URL string into an [`InputSource`].
///
/// # Supported Formats
///
/// | Format | Example | Description |
/// |--------|---------|-------------|
/// | Git HTTPS | `git:https://github.com/org/repo.git` | HTTPS, uses HEAD |
/// | Git HTTPS + ref | `git:https://github.com/org/repo.git#v1.0.0` | HTTPS with specific ref |
/// | Git SSH | `git:git@github.com:org/repo.git` | SSH, uses HEAD |
/// | Git SSH + ref | `git:git@github.com:org/repo.git#main` | SSH with specific ref |
/// | Path absolute | `path:~/code/foo` | Tilde-expanded path |
/// | Path relative | `path:./relative` | Relative to config directory |
///
/// The `#ref` suffix for git URLs can be:
/// - A branch name: `#main`, `#develop`
/// - A tag: `#v1.0.0`, `#release-2024`
/// - A commit hash: `#abc123def` (full or abbreviated)
///
/// # Errors
///
/// Returns [`ParseError`] if the URL format is not recognized or is malformed.
///
/// # Example
///
/// ```
/// use syslua_lib::inputs::source::{parse, InputSource};
///
/// // Git without ref (uses HEAD)
/// let git = parse("git:https://github.com/example/repo.git").unwrap();
/// assert!(matches!(git, InputSource::Git { rev: None, .. }));
///
/// // Git with ref
/// let git_ref = parse("git:https://github.com/example/repo.git#v1.0.0").unwrap();
/// assert!(matches!(git_ref, InputSource::Git { rev: Some(_), .. }));
///
/// // Path input
/// let path = parse("path:~/dotfiles").unwrap();
/// assert!(matches!(path, InputSource::Path { .. }));
/// ```
pub fn parse(url: &str) -> Result<InputSource, ParseError> {
  if let Some(rest) = url.strip_prefix("git:") {
    if rest.is_empty() {
      return Err(ParseError::MissingGitUrl);
    }

    // Check for #ref suffix
    let (git_url, rev) = if let Some(hash_pos) = rest.rfind('#') {
      let url_part = &rest[..hash_pos];
      let ref_part = &rest[hash_pos + 1..];

      if url_part.is_empty() {
        return Err(ParseError::MissingGitUrl);
      }
      if ref_part.is_empty() {
        return Err(ParseError::EmptyGitRef);
      }

      (url_part.to_string(), Some(ref_part.to_string()))
    } else {
      (rest.to_string(), None)
    };

    Ok(InputSource::Git { url: git_url, rev })
  } else if let Some(rest) = url.strip_prefix("path:") {
    if rest.is_empty() {
      return Err(ParseError::MissingPath);
    }
    Ok(InputSource::Path {
      path: PathBuf::from(rest),
    })
  } else {
    // Extract scheme for error message
    let scheme = url.split(':').next().unwrap_or(url);
    Err(ParseError::UnknownScheme(scheme.to_string()))
  }
}

/// Returns the scheme/type identifier for an [`InputSource`].
///
/// Used for lock file serialization.
pub fn source_type(source: &InputSource) -> &'static str {
  match source {
    InputSource::Git { .. } => "git",
    InputSource::Path { .. } => "path",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod parse_git {
    use super::*;

    #[test]
    fn https_url_no_ref() {
      let result = parse("git:https://github.com/org/repo.git").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "https://github.com/org/repo.git".to_string(),
          rev: None,
        }
      );
    }

    #[test]
    fn https_url_with_tag_ref() {
      let result = parse("git:https://github.com/org/repo.git#v1.0.0").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "https://github.com/org/repo.git".to_string(),
          rev: Some("v1.0.0".to_string()),
        }
      );
    }

    #[test]
    fn https_url_with_branch_ref() {
      let result = parse("git:https://github.com/org/repo.git#main").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "https://github.com/org/repo.git".to_string(),
          rev: Some("main".to_string()),
        }
      );
    }

    #[test]
    fn https_url_with_commit_ref() {
      let result = parse("git:https://github.com/org/repo.git#abc123def456").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "https://github.com/org/repo.git".to_string(),
          rev: Some("abc123def456".to_string()),
        }
      );
    }

    #[test]
    fn ssh_url_no_ref() {
      let result = parse("git:git@github.com:org/repo.git").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "git@github.com:org/repo.git".to_string(),
          rev: None,
        }
      );
    }

    #[test]
    fn ssh_url_with_ref() {
      let result = parse("git:git@github.com:org/repo.git#develop").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "git@github.com:org/repo.git".to_string(),
          rev: Some("develop".to_string()),
        }
      );
    }

    #[test]
    fn gitlab_ssh() {
      let result = parse("git:git@gitlab.com:myorg/myrepo.git").unwrap();
      assert_eq!(
        result,
        InputSource::Git {
          url: "git@gitlab.com:myorg/myrepo.git".to_string(),
          rev: None,
        }
      );
    }

    #[test]
    fn missing_url_after_prefix() {
      let result = parse("git:");
      assert_eq!(result, Err(ParseError::MissingGitUrl));
    }

    #[test]
    fn empty_ref_after_hash() {
      let result = parse("git:https://github.com/org/repo.git#");
      assert_eq!(result, Err(ParseError::EmptyGitRef));
    }

    #[test]
    fn only_hash_no_url() {
      let result = parse("git:#v1.0.0");
      assert_eq!(result, Err(ParseError::MissingGitUrl));
    }
  }

  mod parse_path {
    use super::*;

    #[test]
    fn tilde_path() {
      let result = parse("path:~/dotfiles").unwrap();
      assert_eq!(
        result,
        InputSource::Path {
          path: PathBuf::from("~/dotfiles")
        }
      );
    }

    #[test]
    fn relative_path() {
      let result = parse("path:./local-config").unwrap();
      assert_eq!(
        result,
        InputSource::Path {
          path: PathBuf::from("./local-config")
        }
      );
    }

    #[test]
    fn absolute_path() {
      let result = parse("path:/home/user/code/project").unwrap();
      assert_eq!(
        result,
        InputSource::Path {
          path: PathBuf::from("/home/user/code/project")
        }
      );
    }

    #[test]
    fn missing_path_after_prefix() {
      let result = parse("path:");
      assert_eq!(result, Err(ParseError::MissingPath));
    }
  }

  mod parse_errors {
    use super::*;

    #[test]
    fn unknown_scheme() {
      let result = parse("http://example.com");
      assert_eq!(result, Err(ParseError::UnknownScheme("http".to_string())));
    }

    #[test]
    fn no_scheme() {
      let result = parse("just-a-string");
      assert_eq!(result, Err(ParseError::UnknownScheme("just-a-string".to_string())));
    }

    #[test]
    fn empty_string() {
      let result = parse("");
      assert_eq!(result, Err(ParseError::UnknownScheme("".to_string())));
    }
  }

  mod source_type_fn {
    use super::*;

    #[test]
    fn git_type() {
      let source = InputSource::Git {
        url: "https://example.com".to_string(),
        rev: None,
      };
      assert_eq!(source_type(&source), "git");
    }

    #[test]
    fn git_type_with_rev() {
      let source = InputSource::Git {
        url: "https://example.com".to_string(),
        rev: Some("v1.0.0".to_string()),
      };
      assert_eq!(source_type(&source), "git");
    }

    #[test]
    fn path_type() {
      let source = InputSource::Path {
        path: PathBuf::from("/foo"),
      };
      assert_eq!(source_type(&source), "path");
    }
  }
}
