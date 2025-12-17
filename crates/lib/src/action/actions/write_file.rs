use std::path::Path;

use tokio::fs;
use tracing::info;

use crate::execute::ExecuteError;

pub async fn execute_write_file(path: &Path, content: &str) -> Result<(), ExecuteError> {
  info!(path = ?path, "writing file");

  // Create parent directories if they don't exist
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }

  // Write content to file
  fs::write(path, content).await?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  #[tokio::test]
  async fn test_execute_write_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("subdir/test.txt");
    let content = "Hello, world!";

    execute_write_file(&file_path, content).await.unwrap();

    let written_content = fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(written_content, content);
  }
}
