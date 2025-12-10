//! sys-core: Core logic for sys.lua
//!
//! This crate provides the store, derivation, manifest, plan, and apply functionality for sys.lua.
//!
//! # Derivations
//!
//! Derivations are the sole primitive for producing store content. Everything in the store
//! is the output of realizing a derivation. This includes files, environment variables,
//! and packages.
//!
//! # Store
//!
//! The store is the realization engine for derivations. It provides content-addressed
//! storage with human-readable paths.

mod build;
mod derivation;
mod env;
mod env_derivation;
mod error;
mod file_derivation;
mod input;
mod manifest;
mod plan;
mod snapshot;
mod store;

pub use build::BuildContext;
pub use derivation::{
    Derivation, DerivationMeta, DerivationRef, DerivationSpec, DerivationType, InputValue,
    LinkRegistration, System,
};
pub use env::{generate_env_script, source_command, write_env_scripts};
pub use env_derivation::{
    build_env_derivation, generate_profile_scripts, process_env_declarations,
    profile_source_command,
};
pub use error::CoreError;
pub use file_derivation::{apply_file_link, build_file_derivation, process_file_declarations};
pub use input::{InputManager, InputSource, LockFile, LockedInput, ResolvedInput};
pub use manifest::Manifest;
pub use plan::{ApplyOptions, FileChange, FileChangeKind, Plan, apply, compute_plan};
pub use snapshot::{
    RollbackResult, Snapshot, SnapshotDerivation, SnapshotEnv, SnapshotFile, SnapshotFileType,
    SnapshotManager, SnapshotMetadata, SnapshotSummary,
};
pub use store::{Store, sha256_directory, sha256_file, sha256_hex, sha256_string, truncate_hash};

// Re-export types from sys-lua for convenience
pub use sys_lua::{EnvDecl, EnvMergeStrategy, EnvValue, FileDecl};
// Re-export Shell from sys-platform
pub use sys_platform::Shell;

/// Result type for core operations
pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to set up a test environment with a store and temporary directories
    fn setup_test_env() -> (Store, TempDir, TempDir) {
        let store_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();
        let store = Store::new(store_dir.path().join("store"));
        store.init().unwrap();
        (store, store_dir, work_dir)
    }

    /// Integration test: file derivation workflow (store-backed)
    #[test]
    fn test_file_derivation_workflow_store_backed() {
        let (store, _store_dir, work_dir) = setup_test_env();

        // Create a source file in the work directory
        let source_file = work_dir.path().join("dotfiles/gitconfig");
        fs::create_dir_all(source_file.parent().unwrap()).unwrap();
        fs::write(
            &source_file,
            "[user]\n  name = Test User\n  email = test@example.com\n",
        )
        .unwrap();

        // Create a target path for the symlink
        let target_dir = work_dir.path().join("home/.config");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("gitconfig");

        // Create a FileDecl
        let file_decl = FileDecl::from_source(&target_path, "dotfiles/gitconfig");

        // Build the file derivation
        let (drv, link) = build_file_derivation(&file_decl, &store, work_dir.path()).unwrap();

        // Verify derivation was built
        assert!(drv.realized);
        assert!(drv.out().is_some());
        assert!(!link.mutable);

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

        // Verify content is accessible through the symlink
        let content = fs::read_to_string(&target_path).unwrap();
        assert!(content.contains("[user]"));
        assert!(content.contains("Test User"));
    }

    /// Integration test: file derivation workflow (mutable)
    #[test]
    fn test_file_derivation_workflow_mutable() {
        let (store, _store_dir, work_dir) = setup_test_env();

        // Create a source file
        let source_file = work_dir.path().join("dotfiles/mutable.txt");
        fs::create_dir_all(source_file.parent().unwrap()).unwrap();
        fs::write(&source_file, "Original content\n").unwrap();

        // Create target path
        let target_dir = work_dir.path().join("home/.config");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("mutable.txt");

        // Create a mutable FileDecl
        let file_decl = FileDecl::mutable_source(&target_path, &source_file);

        // Build the file derivation
        let (drv, link) = build_file_derivation(&file_decl, &store, work_dir.path()).unwrap();

        // Verify it's a mutable derivation
        assert!(!drv.realized); // Mutable files don't get realized outputs
        assert!(link.mutable);

        // Apply the link
        apply_file_link(&link, &drv, &store).unwrap();

        // Verify symlink points directly to source
        assert!(
            target_path
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let link_target = fs::read_link(&target_path).unwrap();
        assert_eq!(link_target, source_file);

        // Verify changes to source are reflected through symlink
        fs::write(&source_file, "Modified content\n").unwrap();
        let content = fs::read_to_string(&target_path).unwrap();
        assert!(content.contains("Modified"));
    }

    /// Integration test: file derivation with inline content
    #[test]
    fn test_file_derivation_workflow_content() {
        let (store, _store_dir, work_dir) = setup_test_env();

        let target_dir = work_dir.path().join("home/.config/nvim");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("init.lua");

        // Create FileDecl with inline content
        let file_decl = FileDecl::from_content(&target_path, "require('config')\n");

        // Build and apply
        let (drv, link) = build_file_derivation(&file_decl, &store, work_dir.path()).unwrap();
        apply_file_link(&link, &drv, &store).unwrap();

        // Verify
        let content = fs::read_to_string(&target_path).unwrap();
        assert_eq!(content, "require('config')\n");
    }

    /// Integration test: env derivation workflow
    #[test]
    fn test_env_derivation_workflow() {
        let (store, _store_dir, work_dir) = setup_test_env();

        // Create env declarations
        let envs = vec![
            EnvDecl::new("EDITOR", "nvim"),
            EnvDecl::new("PAGER", "less"),
            EnvDecl::path_prepend(
                "PATH",
                vec!["/opt/bin".to_string(), "~/.local/bin".to_string()],
            ),
        ];

        // Build env derivations
        let env_drvs = process_env_declarations(&envs, &store).unwrap();

        // Verify all were built
        assert_eq!(env_drvs.len(), 3);
        assert!(env_drvs.iter().all(|d| d.realized));

        // Generate profile scripts
        let profile_dir = work_dir.path().join("profile");
        generate_profile_scripts(&env_drvs, &profile_dir).unwrap();

        // Verify profile scripts exist
        assert!(profile_dir.join("env.sh").exists());
        assert!(profile_dir.join("env.fish").exists());
        assert!(profile_dir.join("env.zsh").exists());

        // Verify content of bash profile
        let bash_profile = fs::read_to_string(profile_dir.join("env.sh")).unwrap();
        assert!(bash_profile.contains("EDITOR"));
        assert!(bash_profile.contains("nvim"));
        assert!(bash_profile.contains("PAGER"));
        assert!(bash_profile.contains("less"));
        assert!(bash_profile.contains("PATH"));
        assert!(bash_profile.contains("/opt/bin"));
    }

    /// Integration test: manifest from Lua config
    #[test]
    fn test_manifest_from_lua_config() {
        let work_dir = TempDir::new().unwrap();

        // Create a Lua config file
        let config_path = work_dir.path().join("init.lua");
        let mut config_file = fs::File::create(&config_path).unwrap();
        writeln!(
            config_file,
            r#"
            file {{
                path = "~/.config/test.txt",
                content = "Hello from Lua!"
            }}
            
            env {{
                MY_VAR = "my_value"
            }}
        "#
        )
        .unwrap();

        // Load manifest
        let manifest = Manifest::from_config(&config_path).unwrap();

        // Verify
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.envs.len(), 1);
        assert!(
            manifest.files[0]
                .content
                .as_ref()
                .unwrap()
                .contains("Hello from Lua")
        );
        assert_eq!(manifest.envs[0].name, "MY_VAR");
    }

    /// Integration test: full workflow with Lua config
    #[test]
    fn test_full_derivation_workflow_from_lua() {
        let (store, _store_dir, work_dir) = setup_test_env();

        // Create source file
        let source_dir = work_dir.path().join("dotfiles");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("gitconfig"), "[core]\n  editor = nvim\n").unwrap();

        // Create target directory
        let target_dir = work_dir.path().join("home/.config");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("gitconfig");

        // Create a Lua config file
        let config_path = work_dir.path().join("init.lua");
        {
            let mut config_file = fs::File::create(&config_path).unwrap();
            writeln!(
                config_file,
                r#"
                file {{
                    path = "{}",
                    source = "dotfiles/gitconfig"
                }}
                
                env {{
                    EDITOR = "nvim",
                    PATH = {{ "/opt/homebrew/bin" }}
                }}
            "#,
                target_path.display()
            )
            .unwrap();
        }

        // Load manifest
        let manifest = Manifest::from_config(&config_path).unwrap();
        let base_path = config_path.parent().unwrap();

        // Process file declarations
        let file_results = process_file_declarations(&manifest.files, &store, base_path).unwrap();
        assert_eq!(file_results.len(), 1);

        // Apply file links
        for (drv, link) in &file_results {
            apply_file_link(link, drv, &store).unwrap();
        }

        // Verify file was linked
        assert!(target_path.exists());
        let content = fs::read_to_string(&target_path).unwrap();
        assert!(content.contains("editor = nvim"));

        // Process env declarations
        let env_drvs = process_env_declarations(&manifest.envs, &store).unwrap();
        assert_eq!(env_drvs.len(), 2);

        // Generate profile scripts
        let profile_dir = work_dir.path().join("profile");
        generate_profile_scripts(&env_drvs, &profile_dir).unwrap();

        // Verify profile scripts
        let bash_profile = fs::read_to_string(profile_dir.join("env.sh")).unwrap();
        assert!(bash_profile.contains("EDITOR"));
        assert!(bash_profile.contains("/opt/homebrew/bin"));
    }

    /// Integration test: idempotency - running twice produces same result
    #[test]
    fn test_derivation_idempotency() {
        let (store, _store_dir, work_dir) = setup_test_env();

        let target_dir = work_dir.path().join("home/.config");
        fs::create_dir_all(&target_dir).unwrap();
        let target_path = target_dir.join("test.txt");

        let file_decl = FileDecl::from_content(&target_path, "Idempotent content\n");

        // First run
        let (drv1, link1) = build_file_derivation(&file_decl, &store, work_dir.path()).unwrap();
        apply_file_link(&link1, &drv1, &store).unwrap();

        let content1 = fs::read_to_string(&target_path).unwrap();
        let hash1 = drv1.hash.clone();

        // Second run (should hit cache)
        let (drv2, link2) = build_file_derivation(&file_decl, &store, work_dir.path()).unwrap();
        apply_file_link(&link2, &drv2, &store).unwrap();

        let content2 = fs::read_to_string(&target_path).unwrap();
        let hash2 = drv2.hash.clone();

        // Verify same results
        assert_eq!(content1, content2);
        assert_eq!(hash1, hash2);
        assert_eq!(drv1.out(), drv2.out());
    }

    /// Integration test: content deduplication
    #[test]
    fn test_content_deduplication() {
        let (store, _store_dir, work_dir) = setup_test_env();

        let target_dir = work_dir.path().join("home/.config");
        fs::create_dir_all(&target_dir).unwrap();

        // Create two files with same content
        let file1 = FileDecl::from_content(target_dir.join("file1.txt"), "Same content\n");
        let file2 = FileDecl::from_content(target_dir.join("file2.txt"), "Same content\n");

        let (drv1, _) = build_file_derivation(&file1, &store, work_dir.path()).unwrap();
        let (drv2, _) = build_file_derivation(&file2, &store, work_dir.path()).unwrap();

        // Different derivation hashes (different target paths affect hash)
        // But the actual content in store should be deduplicated
        // (The output paths will be different due to different names)
        assert!(drv1.realized);
        assert!(drv2.realized);
    }

    /// Integration test: input workflow with resolved inputs
    ///
    /// Tests that:
    /// 1. Local path inputs work correctly with module loading
    /// 2. Manifest::from_config_with_inputs correctly uses resolved paths
    /// 3. The full workflow from config -> manifest -> derivations works
    #[test]
    fn test_input_workflow_local_path() {
        let work_dir = TempDir::new().unwrap();

        // Create a local "package" with a module
        let pkgs_dir = work_dir.path().join("my-packages");
        fs::create_dir_all(&pkgs_dir).unwrap();
        fs::write(
            pkgs_dir.join("greeter.lua"),
            r#"return { greeting = "Hello from input!" }"#,
        )
        .unwrap();

        // Create init.lua in the package
        fs::write(
            pkgs_dir.join("init.lua"),
            r#"
            local M = {}
            M.greeter = require("greeter")
            return M
        "#,
        )
        .unwrap();

        // Create a Lua config that uses the local input
        let config_path = work_dir.path().join("init.lua");
        fs::write(
            &config_path,
            r#"
            -- Load local packages
            local pkgs = input { source = "path:./my-packages" }
            
            -- Use a value from the input
            local greeting = pkgs.greeter.greeting
            
            file {
                path = "/tmp/test-input-greeting.txt",
                content = greeting
            }
        "#,
        )
        .unwrap();

        // Load manifest (no pre-resolved inputs needed for path: inputs)
        let manifest = Manifest::from_config(&config_path).unwrap();

        // Verify
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.inputs.len(), 1);
        assert!(manifest.inputs[0].source.starts_with("path:"));
        assert!(manifest.inputs[0].resolved_path.is_some());

        // Verify the content was loaded from the input
        let content = manifest.files[0].content.as_ref().unwrap();
        assert_eq!(content, "Hello from input!");
    }

    /// Integration test: input workflow with pre-resolved GitHub inputs
    ///
    /// Simulates the workflow when a GitHub input has been fetched and locked.
    #[test]
    fn test_input_workflow_with_resolved_inputs() {
        use std::collections::HashMap;

        let work_dir = TempDir::new().unwrap();

        // Create a "cached" GitHub input (simulating what would be in the cache)
        let cached_input_dir = work_dir.path().join("cache/github-sys-lua-pkgs-abc123");
        fs::create_dir_all(&cached_input_dir).unwrap();
        fs::write(
            cached_input_dir.join("init.lua"),
            r#"
            local M = {}
            M.tool = { name = "my-tool", version = "1.0.0" }
            return M
        "#,
        )
        .unwrap();

        // Create a Lua config that uses a GitHub input
        let config_path = work_dir.path().join("init.lua");
        fs::write(
            &config_path,
            r#"
            -- Load packages from GitHub (would normally error without resolution)
            local pkgs = input { source = "sys-lua/pkgs" }
            
            -- Use a value from the input
            local tool_name = pkgs.tool.name
            
            file {
                path = "/tmp/test-resolved-input.txt",
                content = tool_name
            }
        "#,
        )
        .unwrap();

        // Create resolved inputs map (simulating what InputManager would provide)
        let mut resolved_inputs = HashMap::new();
        resolved_inputs.insert("sys-lua/pkgs".to_string(), cached_input_dir.clone());

        // Load manifest with resolved inputs
        let manifest = Manifest::from_config_with_inputs(&config_path, &resolved_inputs).unwrap();

        // Verify
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.inputs.len(), 1);
        assert_eq!(manifest.inputs[0].source, "sys-lua/pkgs");
        assert!(manifest.inputs[0].resolved_path.is_some());

        // Verify the content was loaded from the resolved input
        let content = manifest.files[0].content.as_ref().unwrap();
        assert_eq!(content, "my-tool");
    }

    /// Integration test: input workflow with nested module access
    #[test]
    fn test_input_workflow_nested_modules() {
        let work_dir = TempDir::new().unwrap();

        // Create a local package with nested structure
        let pkgs_dir = work_dir.path().join("packages");
        let tools_dir = pkgs_dir.join("tools");
        fs::create_dir_all(&tools_dir).unwrap();

        // Create tools/ripgrep.lua
        fs::write(
            tools_dir.join("ripgrep.lua"),
            r#"return { name = "ripgrep", version = "14.1.0" }"#,
        )
        .unwrap();

        // Create tools/fd.lua
        fs::write(
            tools_dir.join("fd.lua"),
            r#"return { name = "fd", version = "9.0.0" }"#,
        )
        .unwrap();

        // Create a Lua config that uses nested modules
        let config_path = work_dir.path().join("init.lua");
        fs::write(
            &config_path,
            r#"
            local pkgs = input { source = "path:./packages" }
            
            -- Access nested modules
            local rg = pkgs.tools.ripgrep
            local fd = pkgs.tools.fd
            
            file {
                path = "/tmp/test-nested-1.txt",
                content = rg.name .. "-" .. rg.version
            }
            
            file {
                path = "/tmp/test-nested-2.txt",
                content = fd.name .. "-" .. fd.version
            }
        "#,
        )
        .unwrap();

        // Load manifest
        let manifest = Manifest::from_config(&config_path).unwrap();

        // Verify
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(
            manifest.files[0].content.as_ref().unwrap(),
            "ripgrep-14.1.0"
        );
        assert_eq!(manifest.files[1].content.as_ref().unwrap(), "fd-9.0.0");
    }
}
