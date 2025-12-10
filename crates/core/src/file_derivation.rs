//! File derivations for sys.lua
//!
//! File derivations transform `FileDecl` entries into derivations that:
//! - **Store-backed (default)**: Copy content into the store, symlink to target path
//! - **Mutable**: Create direct symlink from target to source, metadata in store
//!
//! # Store-backed mode
//!
//! ```lua
//! file { path = "~/.gitconfig", source = "./dotfiles/gitconfig" }
//! file { path = "~/.config/nvim/init.lua", content = [[require("config")]] }
//! ```
//!
//! The content (from source file or inline content) is:
//! 1. Hashed to compute the derivation hash
//! 2. Copied into the store at `obj/file-<target_name>-<hash>/content`
//! 3. Symlinked from target path to the store path
//!
//! # Mutable mode
//!
//! ```lua
//! file { path = "~/.gitconfig", source = "./dotfiles/gitconfig", mutable = true }
//! ```
//!
//! The source file is:
//! 1. Symlinked directly from target to source (no store copy)
//! 2. Metadata recorded in store at `drv/<hash>.drv`

use crate::Result;
use crate::derivation::{Derivation, DerivationSpec, InputValue, LinkRegistration, System};
use crate::error::CoreError;
use crate::store::{Store, sha256_file, sha256_string};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use sys_lua::FileDecl;
use tracing::{debug, info};

/// Build a file derivation from a FileDecl.
///
/// This creates:
/// - A `DerivationSpec` describing the file
/// - A `LinkRegistration` connecting the derivation output to the target path
///
/// For store-backed files, the content is copied into the store.
/// For mutable files, only metadata is stored.
pub fn build_file_derivation(
    decl: &FileDecl,
    store: &Store,
    base_path: &Path,
) -> Result<(Derivation, LinkRegistration)> {
    // Validate the declaration
    decl.validate().map_err(CoreError::InvalidDerivationSpec)?;

    // Determine the file name for the derivation
    let target_name = decl
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    // Build the derivation based on the mode
    if decl.is_mutable() {
        build_mutable_file_derivation(decl, store, base_path, target_name)
    } else {
        build_store_backed_file_derivation(decl, store, base_path, target_name)
    }
}

/// Build a store-backed file derivation.
///
/// The content is copied into the store at `obj/file-<name>-<hash>/content`.
fn build_store_backed_file_derivation(
    decl: &FileDecl,
    store: &Store,
    base_path: &Path,
    target_name: &str,
) -> Result<(Derivation, LinkRegistration)> {
    // Get the content (from source file or inline content)
    let (content, content_hash) = get_content_and_hash(decl, base_path)?;

    // Build inputs
    let mut inputs = BTreeMap::new();
    inputs.insert("type".to_string(), InputValue::String("file".to_string()));
    inputs.insert(
        "target".to_string(),
        InputValue::String(decl.path.display().to_string()),
    );
    inputs.insert(
        "content_hash".to_string(),
        InputValue::String(content_hash.clone()),
    );

    if let Some(mode) = decl.mode {
        inputs.insert("mode".to_string(), InputValue::Number(mode as f64));
    }

    // Create derivation spec
    let spec = DerivationSpec {
        name: format!("file-{}", target_name),
        version: None,
        inputs,
        build_hash: content_hash.clone(), // Use content hash as build hash for files
        outputs: vec!["out".to_string()],
        system: System::current(),
    };

    let drv = Derivation::new(spec);

    // Build the output in the store
    let output_path = realize_store_backed_file(store, &drv, &content, decl.mode)?;

    // Create a derivation with the output path set
    let mut realized_drv = drv;
    realized_drv
        .output_paths
        .insert("out".to_string(), output_path);
    realized_drv.realized = true;

    // Create link registration
    let link = LinkRegistration {
        derivation_hash: realized_drv.hash.clone(),
        output: "out".to_string(),
        target: decl.path.clone(),
        mutable: false,
        source_subpath: Some("content".to_string()),
    };

    info!(
        "Built store-backed file derivation: {} -> {}",
        target_name,
        realized_drv.short_hash()
    );

    Ok((realized_drv, link))
}

/// Build a mutable file derivation.
///
/// Only metadata is stored; the file remains a direct symlink to the source.
fn build_mutable_file_derivation(
    decl: &FileDecl,
    store: &Store,
    base_path: &Path,
    target_name: &str,
) -> Result<(Derivation, LinkRegistration)> {
    // Get the effective source path
    let source = decl.effective_source().ok_or_else(|| {
        CoreError::InvalidDerivationSpec("Mutable file requires source".to_string())
    })?;

    // Resolve the source path relative to base_path
    let resolved_source = if source.is_absolute() {
        source.clone()
    } else {
        base_path.join(source)
    };

    // Compute hash from the source path (not content, for stability)
    let source_hash = sha256_string(&resolved_source.display().to_string());

    // Build inputs
    let mut inputs = BTreeMap::new();
    inputs.insert("type".to_string(), InputValue::String("file".to_string()));
    inputs.insert(
        "target".to_string(),
        InputValue::String(decl.path.display().to_string()),
    );
    inputs.insert(
        "source".to_string(),
        InputValue::String(resolved_source.display().to_string()),
    );
    inputs.insert("mutable".to_string(), InputValue::Bool(true));

    if let Some(mode) = decl.mode {
        inputs.insert("mode".to_string(), InputValue::Number(mode as f64));
    }

    // Create derivation spec
    let spec = DerivationSpec {
        name: format!("file-{}", target_name),
        version: None,
        inputs,
        build_hash: source_hash, // Use source path hash for mutable files
        outputs: vec!["out".to_string()],
        system: System::current(),
    };

    let drv = Derivation::new(spec);

    // Save derivation metadata (no output to realize)
    store.save_derivation(&drv)?;

    // Create link registration pointing directly to the source
    let link = LinkRegistration {
        derivation_hash: drv.hash.clone(),
        output: "out".to_string(),
        target: decl.path.clone(),
        mutable: true,
        source_subpath: None,
    };

    info!(
        "Built mutable file derivation: {} -> {} (source: {})",
        target_name,
        drv.short_hash(),
        resolved_source.display()
    );

    Ok((drv, link))
}

/// Get the content and its hash from a FileDecl.
fn get_content_and_hash(decl: &FileDecl, base_path: &Path) -> Result<(Vec<u8>, String)> {
    if let Some(content) = &decl.content {
        // Inline content
        let hash = sha256_string(content);
        Ok((content.as_bytes().to_vec(), hash))
    } else if let Some(source) = decl.effective_source() {
        // Source file
        let resolved = if source.is_absolute() {
            source.clone()
        } else {
            base_path.join(source)
        };

        if !resolved.exists() {
            return Err(CoreError::FileOperation {
                path: resolved.display().to_string(),
                message: "Source file does not exist".to_string(),
            });
        }

        let content = fs::read(&resolved)?;
        let hash = sha256_file(&resolved)?;
        Ok((content, hash))
    } else {
        Err(CoreError::InvalidDerivationSpec(
            "FileDecl has no content or source".to_string(),
        ))
    }
}

/// Realize a store-backed file derivation.
///
/// Creates the output directory in the store with the file content.
fn realize_store_backed_file(
    store: &Store,
    drv: &Derivation,
    content: &[u8],
    mode: Option<u32>,
) -> Result<PathBuf> {
    // Check if already realized via cache
    if let Some(output_hash) = store.lookup_cache(&drv.hash) {
        let path = store.object_path(drv.name(), drv.version(), &output_hash);
        if path.exists() {
            debug!("File derivation {} already realized", drv.short_hash());
            return Ok(path);
        }
    }

    // Create temporary build output
    let temp_dir = tempfile::tempdir()?;
    let content_path = temp_dir.path().join("content");

    // Write content
    fs::write(&content_path, content)?;

    // Set permissions if specified
    if let Some(mode) = mode {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&content_path, fs::Permissions::from_mode(mode))?;
        }
    }

    // Finalize to store
    let output_path = store.finalize_output(drv, temp_dir.path())?;

    // Save derivation spec
    store.save_derivation(drv)?;

    Ok(output_path)
}

/// Apply a file link registration.
///
/// Creates the symlink from target to the store object (or source for mutable).
pub fn apply_file_link(link: &LinkRegistration, drv: &Derivation, _store: &Store) -> Result<()> {
    let target = &link.target;

    // Create parent directories
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    // Remove existing file/symlink
    if target.symlink_metadata().is_ok() {
        if target.is_dir() && !target.symlink_metadata()?.file_type().is_symlink() {
            fs::remove_dir_all(target)?;
        } else {
            fs::remove_file(target)?;
        }
    }

    // Determine link target
    let link_target = if link.mutable {
        // For mutable files, link directly to source
        drv.spec
            .inputs
            .get("source")
            .and_then(|v| match v {
                InputValue::String(s) => Some(PathBuf::from(s)),
                _ => None,
            })
            .ok_or_else(|| {
                CoreError::InvalidDerivationSpec("Mutable derivation missing source".to_string())
            })?
    } else {
        // For store-backed files, link to store path + subpath
        let output_path = drv.out().ok_or_else(|| {
            CoreError::InvalidDerivationSpec("Derivation has no output path".to_string())
        })?;

        if let Some(subpath) = &link.source_subpath {
            output_path.join(subpath)
        } else {
            output_path.clone()
        }
    };

    // Create symlink
    #[cfg(unix)]
    std::os::unix::fs::symlink(&link_target, target)?;

    #[cfg(windows)]
    {
        if link_target.is_dir() {
            std::os::windows::fs::symlink_dir(&link_target, target)?;
        } else {
            std::os::windows::fs::symlink_file(&link_target, target)?;
        }
    }

    info!("Linked {} -> {}", target.display(), link_target.display());

    Ok(())
}

/// Build and apply file derivations from a manifest.
///
/// Returns the list of created derivations and their link registrations.
pub fn process_file_declarations(
    files: &[FileDecl],
    store: &Store,
    base_path: &Path,
) -> Result<Vec<(Derivation, LinkRegistration)>> {
    let mut results = Vec::new();

    for decl in files {
        let (drv, link) = build_file_derivation(decl, store, base_path)?;
        results.push((drv, link));
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_store() -> (Store, TempDir) {
        let temp = TempDir::new().unwrap();
        let store = Store::new(temp.path().join("store"));
        store.init().unwrap();
        (store, temp)
    }

    #[test]
    fn test_store_backed_file_from_content() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        let decl = FileDecl::from_content("/home/user/.config/test.txt", "Hello, World!");

        let (drv, link) = build_file_derivation(&decl, &store, base_path).unwrap();

        assert!(drv.realized);
        assert!(drv.out().is_some());
        assert!(!link.mutable);
        assert_eq!(link.source_subpath, Some("content".to_string()));

        // Verify content in store
        let content_path = drv.out().unwrap().join("content");
        assert!(content_path.exists());
        assert_eq!(fs::read_to_string(&content_path).unwrap(), "Hello, World!");
    }

    #[test]
    fn test_store_backed_file_from_source() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        // Create source file
        let source_path = base_path.join("source.txt");
        fs::write(&source_path, "Source content").unwrap();

        let decl = FileDecl::from_source("/home/user/.config/test.txt", "source.txt");

        let (drv, link) = build_file_derivation(&decl, &store, base_path).unwrap();

        assert!(drv.realized);
        assert!(!link.mutable);

        // Verify content in store
        let content_path = drv.out().unwrap().join("content");
        assert_eq!(fs::read_to_string(&content_path).unwrap(), "Source content");
    }

    #[test]
    fn test_mutable_file() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        // Create source file
        let source_path = base_path.join("source.txt");
        fs::write(&source_path, "Mutable content").unwrap();

        let decl = FileDecl::mutable_source("/home/user/.config/test.txt", "source.txt");

        let (drv, link) = build_file_derivation(&decl, &store, base_path).unwrap();

        // Mutable files don't get realized outputs
        assert!(!drv.realized);
        assert!(link.mutable);

        // Verify derivation was saved
        let loaded = store.load_derivation(&drv.hash).unwrap();
        assert_eq!(loaded.name, drv.spec.name);
    }

    #[test]
    fn test_apply_store_backed_link() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        // Create the target directory
        let target_dir = temp.path().join("home/user/.config");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("test.txt");

        let decl = FileDecl::from_content(&target_path, "Linked content");

        let (drv, link) = build_file_derivation(&decl, &store, base_path).unwrap();

        // Apply the link
        apply_file_link(&link, &drv, &store).unwrap();

        // Verify the symlink was created
        assert!(target_path.exists());
        assert!(
            target_path
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&target_path).unwrap(), "Linked content");
    }

    #[test]
    fn test_apply_mutable_link() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        // Create source file
        let source_path = base_path.join("source.txt");
        fs::write(&source_path, "Mutable content").unwrap();

        // Create target directory
        let target_dir = temp.path().join("home/user/.config");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("test.txt");

        let mut decl = FileDecl::mutable_source(&target_path, &source_path);
        // Use absolute source path for test
        decl.source = Some(source_path.clone());

        let (drv, link) = build_file_derivation(&decl, &store, base_path).unwrap();

        // Apply the link
        apply_file_link(&link, &drv, &store).unwrap();

        // Verify the symlink was created directly to source
        assert!(target_path.exists());
        assert!(
            target_path
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_link(&target_path).unwrap(), source_path);
        assert_eq!(fs::read_to_string(&target_path).unwrap(), "Mutable content");
    }

    #[test]
    fn test_file_derivation_caching() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        let decl = FileDecl::from_content("/home/user/.config/test.txt", "Cached content");

        // Build twice - second should hit cache
        let (drv1, _) = build_file_derivation(&decl, &store, base_path).unwrap();
        let (drv2, _) = build_file_derivation(&decl, &store, base_path).unwrap();

        // Same hash means same derivation
        assert_eq!(drv1.hash, drv2.hash);
        assert_eq!(drv1.out(), drv2.out());
    }

    #[test]
    fn test_file_with_mode() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        let mut decl = FileDecl::from_content("/home/user/.local/bin/script", "#!/bin/sh\necho hi");
        decl.mode = Some(0o755);

        let (drv, _) = build_file_derivation(&decl, &store, base_path).unwrap();

        // Verify the content file has correct permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let content_path = drv.out().unwrap().join("content");
            let perms = fs::metadata(&content_path).unwrap().permissions();
            // Note: Store makes files read-only, so we check for the base mode minus write
            // The original mode is 755, but after make_immutable it becomes 555
            assert_eq!(perms.mode() & 0o777, 0o555);
        }
    }

    #[test]
    fn test_process_file_declarations() {
        let (store, temp) = setup_store();
        let base_path = temp.path();

        // Create source file
        let source_path = base_path.join("source.txt");
        fs::write(&source_path, "Source").unwrap();

        let files = vec![
            FileDecl::from_content("/home/user/.config/a.txt", "Content A"),
            FileDecl::from_source("/home/user/.config/b.txt", "source.txt"),
        ];

        let results = process_file_declarations(&files, &store, base_path).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].0.realized);
        assert!(results[1].0.realized);
    }
}
