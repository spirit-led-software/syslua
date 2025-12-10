use anyhow::Result;
use clap::{Parser, Subcommand};
use console::{Term, style};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use sys_core::{
    ApplyOptions, FileChangeKind, InputManager, InputSource, Manifest, Plan, Shell, Snapshot,
    SnapshotDerivation, SnapshotEnv, SnapshotFile, SnapshotManager, Store, apply, apply_file_link,
    compute_plan, generate_env_script, generate_profile_scripts, process_env_declarations,
    process_file_declarations, profile_source_command, source_command, write_env_scripts,
};
use sys_platform::Platform;
use tracing_subscriber::EnvFilter;

// Helper to convert CoreError to anyhow::Error (works around mlua not being Send+Sync)
fn map_core_err<T>(result: sys_core::Result<T>) -> Result<T> {
    result.map_err(|e| anyhow::anyhow!("{}", e))
}

/// sys.lua - Declarative system/environment manager
#[derive(Parser)]
#[command(name = "sys")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply a configuration file
    Apply {
        /// Path to the configuration file (default: init.lua)
        #[arg(default_value = "init.lua")]
        config: PathBuf,

        /// Force overwrite of existing files
        #[arg(short, long)]
        force: bool,

        /// Use legacy (non-derivation) workflow
        #[arg(long)]
        legacy: bool,
    },

    /// Show what changes would be made (dry-run)
    Plan {
        /// Path to the configuration file (default: init.lua)
        #[arg(default_value = "init.lua")]
        config: PathBuf,

        /// Use legacy (non-derivation) workflow
        #[arg(long)]
        legacy: bool,
    },

    /// Build derivations without applying links
    Build {
        /// Path to the configuration file (default: init.lua)
        #[arg(default_value = "init.lua")]
        config: PathBuf,
    },

    /// Print shell environment activation command
    Env {
        /// Path to the configuration file (default: init.lua)
        #[arg(default_value = "init.lua")]
        config: PathBuf,

        /// Shell to generate script for (auto-detected if not specified)
        #[arg(short, long)]
        shell: Option<String>,

        /// Print the script content instead of source command
        #[arg(long)]
        print: bool,
    },

    /// Show current status
    Status,

    /// Update inputs to their latest versions
    Update {
        /// Specific input to update (updates all if not specified)
        #[arg()]
        input: Option<String>,

        /// Path to the configuration file (default: init.lua)
        #[arg(short, long, default_value = "init.lua")]
        config: PathBuf,
    },

    /// Show snapshot history
    History {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Rollback to a previous snapshot
    Rollback {
        /// Snapshot ID to rollback to (uses previous if not specified)
        #[arg()]
        snapshot_id: Option<String>,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .without_time()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Apply {
            config,
            force,
            legacy,
        } => {
            if legacy {
                cmd_apply_legacy(&config, force, cli.verbose)
            } else {
                cmd_apply(&config, force, cli.verbose)
            }
        }
        Commands::Plan { config, legacy } => {
            if legacy {
                cmd_plan_legacy(&config, cli.verbose)
            } else {
                cmd_plan(&config, cli.verbose)
            }
        }
        Commands::Build { config } => cmd_build(&config, cli.verbose),
        Commands::Env {
            config,
            shell,
            print,
        } => cmd_env(&config, shell, print),
        Commands::Status => cmd_status(),
        Commands::Update { input, config } => cmd_update(&config, input, cli.verbose),
        Commands::History { verbose } => cmd_history(verbose || cli.verbose),
        Commands::Rollback { snapshot_id, yes } => cmd_rollback(snapshot_id, yes, cli.verbose),
    }
}

/// Resolve inputs from lock file before evaluation
///
/// Returns a map of source URI -> local path for all locked inputs
fn resolve_inputs_from_lock(
    config: &Path,
    platform: &Platform,
    term: &Term,
    verbose: bool,
) -> Result<HashMap<String, PathBuf>> {
    let lock_path = config
        .parent()
        .unwrap_or(Path::new("."))
        .join("syslua.lock");

    // If no lock file exists, return empty map
    if !lock_path.exists() {
        return Ok(HashMap::new());
    }

    let cache_dir = platform.input_cache_dir();
    let mut manager = map_core_err(InputManager::new(cache_dir.clone(), lock_path.clone()))?;

    // Collect inputs to resolve (to avoid borrow issues)
    let inputs_to_resolve: Vec<(String, String, InputSource)> = {
        let lock_file = manager.lock_file();
        if lock_file.inputs.is_empty() {
            return Ok(HashMap::new());
        }

        if verbose {
            term.write_line(&format!(
                "{} Loading {} input(s) from lock file",
                style("::").cyan().bold(),
                lock_file.inputs.len()
            ))?;
        }

        lock_file
            .inputs
            .iter()
            .filter_map(|(name, locked)| {
                InputSource::parse(&locked.uri)
                    .ok()
                    .map(|source| (name.clone(), locked.uri.clone(), source))
            })
            .collect()
    };

    let mut resolved = HashMap::new();

    for (name, uri, source) in inputs_to_resolve {
        // Resolve without updating (use locked version)
        match manager.resolve(&name, &source, false) {
            Ok(resolved_input) => {
                if verbose {
                    term.write_line(&format!(
                        "  {} {} -> {}",
                        style("✓").green().bold(),
                        uri,
                        resolved_input.local_path.display()
                    ))?;
                }
                resolved.insert(uri, resolved_input.local_path);
            }
            Err(e) => {
                // Input not cached, suggest running update
                if verbose {
                    term.write_line(&format!(
                        "  {} {} (not cached: {})",
                        style("!").yellow().bold(),
                        uri,
                        e
                    ))?;
                }
            }
        }
    }

    Ok(resolved)
}

/// Apply using the new derivation-based workflow
fn cmd_apply(config: &Path, _force: bool, verbose: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    // Resolve inputs from lock file before evaluation
    let resolved_inputs = resolve_inputs_from_lock(config, &platform, &term, verbose)?;

    term.write_line(&format!(
        "{} Evaluating {}",
        style("::").cyan().bold(),
        config.display()
    ))?;

    // Load manifest with resolved inputs
    let manifest = if resolved_inputs.is_empty() {
        match Manifest::from_config(config) {
            Ok(m) => m,
            Err(e) => {
                term.write_line(&format!(
                    "{} Failed to evaluate config: {}",
                    style("error:").red().bold(),
                    e
                ))?;
                std::process::exit(1);
            }
        }
    } else {
        match Manifest::from_config_with_inputs(config, &resolved_inputs) {
            Ok(m) => m,
            Err(e) => {
                term.write_line(&format!(
                    "{} Failed to evaluate config: {}",
                    style("error:").red().bold(),
                    e
                ))?;
                std::process::exit(1);
            }
        }
    };

    // Initialize store
    let store = Store::new(platform.user_store_path());
    map_core_err(store.init())?;

    // Initialize snapshot manager
    let snapshot_manager = SnapshotManager::new(platform.snapshots_dir());
    map_core_err(snapshot_manager.init())?;

    // Get base path for resolving relative paths
    let base_path = config
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut total_derivations = 0;
    let mut total_links = 0;

    // Prepare snapshot
    let mut snapshot = Snapshot::new(format!("Applied {}", config.display()));
    snapshot = snapshot.with_config(config);

    // Vectors to collect derivations and links for snapshot
    let mut snapshot_files: Vec<SnapshotFile> = Vec::new();
    let mut snapshot_drvs: Vec<SnapshotDerivation> = Vec::new();
    let mut snapshot_envs: Vec<SnapshotEnv> = Vec::new();

    // Process file declarations
    if !manifest.files.is_empty() {
        term.write_line(&format!(
            "{} Building {} file derivation(s)",
            style("::").cyan().bold(),
            manifest.files.len()
        ))?;

        let file_results = map_core_err(process_file_declarations(
            &manifest.files,
            &store,
            &base_path,
        ))?;

        for (drv, link) in &file_results {
            if verbose {
                term.write_line(&format!(
                    "  {} {} -> {}",
                    style("+").green().bold(),
                    link.target.display(),
                    drv.short_hash()
                ))?;
            }

            // Add to snapshot
            let snapshot_file = if link.mutable {
                if let Some(subpath) = &link.source_subpath {
                    SnapshotFile::mutable_symlink(link.target.clone(), PathBuf::from(subpath))
                } else {
                    SnapshotFile::mutable_symlink(link.target.clone(), PathBuf::from(""))
                }
            } else {
                SnapshotFile::store_backed(link.target.clone(), drv.hash.clone(), drv.hash.clone())
            };
            snapshot_files.push(snapshot_file);

            // Add derivation to snapshot
            let snapshot_drv = SnapshotDerivation::new(
                drv.name().to_string(),
                drv.version().map(|s| s.to_string()),
                drv.hash.clone(),
                "file",
            );
            let snapshot_drv = if let Some(out) = drv.out() {
                snapshot_drv.with_output(out.clone())
            } else {
                snapshot_drv
            };
            snapshot_drvs.push(snapshot_drv);
        }

        // Apply file links
        term.write_line(&format!(
            "{} Applying {} file link(s)",
            style("::").cyan().bold(),
            file_results.len()
        ))?;

        for (drv, link) in &file_results {
            map_core_err(apply_file_link(link, drv, &store))?;
            total_links += 1;
        }

        total_derivations += file_results.len();
    }

    // Process env declarations
    if !manifest.envs.is_empty() {
        term.write_line(&format!(
            "{} Building {} env derivation(s)",
            style("::").cyan().bold(),
            manifest.envs.len()
        ))?;

        let env_drvs = map_core_err(process_env_declarations(&manifest.envs, &store))?;

        for (drv, env_decl) in env_drvs.iter().zip(manifest.envs.iter()) {
            if verbose {
                term.write_line(&format!(
                    "  {} {} -> {}",
                    style("+").green().bold(),
                    drv.name(),
                    drv.short_hash()
                ))?;
            }

            // Add env to snapshot
            let value = env_decl
                .values
                .iter()
                .map(|v| v.value.clone())
                .collect::<Vec<_>>()
                .join(":");
            let merge_strategy = if env_decl.values.is_empty() {
                "replace"
            } else {
                match &env_decl.values[0].strategy {
                    sys_core::EnvMergeStrategy::Replace => "replace",
                    sys_core::EnvMergeStrategy::Prepend => "prepend",
                    sys_core::EnvMergeStrategy::Append => "append",
                }
            };
            let snapshot_env = SnapshotEnv::new(env_decl.name.clone(), value, merge_strategy)
                .with_derivation(drv.hash.clone());
            snapshot_envs.push(snapshot_env);

            // Add derivation to snapshot
            let snapshot_drv = SnapshotDerivation::new(
                drv.name().to_string(),
                drv.version().map(|s| s.to_string()),
                drv.hash.clone(),
                "env",
            );
            let snapshot_drv = if let Some(out) = drv.out() {
                snapshot_drv.with_output(out.clone())
            } else {
                snapshot_drv
            };
            snapshot_drvs.push(snapshot_drv);
        }

        // Generate profile scripts
        let profile_dir = platform.profile_dir();
        term.write_line(&format!(
            "{} Generating profile scripts in {}",
            style("::").cyan().bold(),
            profile_dir.display()
        ))?;

        map_core_err(generate_profile_scripts(&env_drvs, &profile_dir))?;

        total_derivations += env_drvs.len();

        // Show source command hint
        let shell = Shell::detect();
        term.write_line("")?;
        term.write_line(&format!(
            "Add to your shell config (~/.{}rc):",
            shell.as_str()
        ))?;
        term.write_line(&format!(
            "  {}",
            style(profile_source_command(&shell, &profile_dir)).cyan()
        ))?;
    }

    // Finalize and save snapshot
    for file in snapshot_files {
        snapshot.add_file(file);
    }
    for drv in snapshot_drvs {
        snapshot.add_derivation(drv);
    }
    for env in snapshot_envs {
        snapshot.add_env(env);
    }

    let snapshot_id = map_core_err(snapshot_manager.create_snapshot(snapshot))?;

    term.write_line("")?;
    term.write_line(&format!(
        "{} Applied {} derivation(s), {} link(s)",
        style("::").green().bold(),
        total_derivations,
        total_links
    ))?;

    if verbose {
        term.write_line(&format!(
            "  Snapshot: {}",
            style(&snapshot_id[..13.min(snapshot_id.len())]).cyan()
        ))?;
    }

    Ok(())
}

/// Plan using the new derivation-based workflow
fn cmd_plan(config: &Path, verbose: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    // Resolve inputs from lock file before evaluation
    let resolved_inputs = resolve_inputs_from_lock(config, &platform, &term, verbose)?;

    term.write_line(&format!(
        "{} Evaluating {}",
        style("::").cyan().bold(),
        config.display()
    ))?;

    // Load manifest with resolved inputs
    let manifest = if resolved_inputs.is_empty() {
        match Manifest::from_config(config) {
            Ok(m) => m,
            Err(e) => {
                term.write_line(&format!(
                    "{} Failed to evaluate config: {}",
                    style("error:").red().bold(),
                    e
                ))?;
                std::process::exit(1);
            }
        }
    } else {
        match Manifest::from_config_with_inputs(config, &resolved_inputs) {
            Ok(m) => m,
            Err(e) => {
                term.write_line(&format!(
                    "{} Failed to evaluate config: {}",
                    style("error:").red().bold(),
                    e
                ))?;
                std::process::exit(1);
            }
        }
    };

    // Initialize store (read-only check, but needed for hash computation)
    let _store = Store::new(platform.user_store_path());

    // Get base path for resolving relative paths
    let _base_path = config
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut has_changes = false;

    // Plan file derivations
    if !manifest.files.is_empty() {
        term.write_line("")?;
        term.write_line(&format!(
            "{} File derivations ({}):",
            style("::").cyan().bold(),
            manifest.files.len()
        ))?;

        for file in &manifest.files {
            // Check if file needs to be created/updated
            let target = &file.path;
            let exists = target.exists() || target.symlink_metadata().is_ok();

            let symbol = if exists {
                // Existing file will be updated/replaced
                style("~").yellow().bold()
            } else {
                style("+").green().bold() // create
            };

            if !exists || verbose {
                has_changes = !exists || has_changes;
                let mode = if file.is_mutable() {
                    "mutable"
                } else {
                    "store-backed"
                };
                term.write_line(&format!("  {} {} ({})", symbol, target.display(), mode))?;
            }
        }
    }

    // Plan env derivations
    if !manifest.envs.is_empty() {
        term.write_line("")?;
        term.write_line(&format!(
            "{} Env derivations ({}):",
            style("::").cyan().bold(),
            manifest.envs.len()
        ))?;

        for env in &manifest.envs {
            term.write_line(&format!("  {} {}", style("+").green().bold(), env.name))?;
        }
        has_changes = true;
    }

    term.write_line("")?;
    if has_changes {
        term.write_line(&format!(
            "{} Would build {} file(s), {} env(s)",
            style("::").cyan().bold(),
            manifest.files.len(),
            manifest.envs.len()
        ))?;
    } else {
        term.write_line(&format!(
            "{} No changes would be made",
            style("::").cyan().bold()
        ))?;
    }

    Ok(())
}

/// Build derivations without applying links
fn cmd_build(config: &Path, verbose: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    // Resolve inputs from lock file before evaluation
    let resolved_inputs = resolve_inputs_from_lock(config, &platform, &term, verbose)?;

    term.write_line(&format!(
        "{} Evaluating {}",
        style("::").cyan().bold(),
        config.display()
    ))?;

    // Load manifest with resolved inputs
    let manifest = if resolved_inputs.is_empty() {
        match Manifest::from_config(config) {
            Ok(m) => m,
            Err(e) => {
                term.write_line(&format!(
                    "{} Failed to evaluate config: {}",
                    style("error:").red().bold(),
                    e
                ))?;
                std::process::exit(1);
            }
        }
    } else {
        match Manifest::from_config_with_inputs(config, &resolved_inputs) {
            Ok(m) => m,
            Err(e) => {
                term.write_line(&format!(
                    "{} Failed to evaluate config: {}",
                    style("error:").red().bold(),
                    e
                ))?;
                std::process::exit(1);
            }
        }
    };

    // Initialize store
    let store = Store::new(platform.user_store_path());
    map_core_err(store.init())?;

    // Get base path for resolving relative paths
    let base_path = config
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut total_derivations = 0;

    // Build file derivations
    if !manifest.files.is_empty() {
        term.write_line(&format!(
            "{} Building {} file derivation(s)",
            style("::").cyan().bold(),
            manifest.files.len()
        ))?;

        let file_results = map_core_err(process_file_declarations(
            &manifest.files,
            &store,
            &base_path,
        ))?;

        for (drv, _link) in &file_results {
            let out_path = drv
                .out()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            term.write_line(&format!(
                "  {} {} -> {}",
                style("✓").green().bold(),
                drv.name(),
                if verbose { &out_path } else { drv.short_hash() }
            ))?;
        }

        total_derivations += file_results.len();
    }

    // Build env derivations
    if !manifest.envs.is_empty() {
        term.write_line(&format!(
            "{} Building {} env derivation(s)",
            style("::").cyan().bold(),
            manifest.envs.len()
        ))?;

        let env_drvs = map_core_err(process_env_declarations(&manifest.envs, &store))?;

        for drv in &env_drvs {
            let out_path = drv
                .out()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            term.write_line(&format!(
                "  {} {} -> {}",
                style("✓").green().bold(),
                drv.name(),
                if verbose { &out_path } else { drv.short_hash() }
            ))?;
        }

        total_derivations += env_drvs.len();
    }

    term.write_line("")?;
    term.write_line(&format!(
        "{} Built {} derivation(s)",
        style("::").green().bold(),
        total_derivations
    ))?;

    Ok(())
}

/// Legacy apply (non-derivation workflow, for backward compatibility)
fn cmd_apply_legacy(config: &Path, force: bool, verbose: bool) -> Result<()> {
    let term = Term::stderr();

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    term.write_line(&format!(
        "{} Evaluating {} (legacy mode)",
        style("::").cyan().bold(),
        config.display()
    ))?;

    // Load manifest
    let manifest = match Manifest::from_config(config) {
        Ok(m) => m,
        Err(e) => {
            term.write_line(&format!(
                "{} Failed to evaluate config: {}",
                style("error:").red().bold(),
                e
            ))?;
            std::process::exit(1);
        }
    };

    // Compute plan
    let plan = map_core_err(compute_plan(&manifest))?;

    if !plan.has_changes() {
        term.write_line(&format!(
            "{} No changes to apply",
            style("::").cyan().bold()
        ))?;
        return Ok(());
    }

    // Show plan
    print_plan(&term, &plan, verbose)?;

    term.write_line("")?;
    term.write_line(&format!(
        "{} Applying {} change(s)",
        style("::").cyan().bold(),
        plan.change_count()
    ))?;

    // Apply
    let options = ApplyOptions {
        force,
        dry_run: false,
    };

    map_core_err(apply(&plan, &options))?;

    term.write_line(&format!("{} Done!", style("::").green().bold()))?;

    Ok(())
}

/// Legacy plan (non-derivation workflow)
fn cmd_plan_legacy(config: &Path, verbose: bool) -> Result<()> {
    let term = Term::stderr();

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    term.write_line(&format!(
        "{} Evaluating {} (legacy mode)",
        style("::").cyan().bold(),
        config.display()
    ))?;

    // Load manifest
    let manifest = match Manifest::from_config(config) {
        Ok(m) => m,
        Err(e) => {
            term.write_line(&format!(
                "{} Failed to evaluate config: {}",
                style("error:").red().bold(),
                e
            ))?;
            std::process::exit(1);
        }
    };

    // Compute plan
    let plan = map_core_err(compute_plan(&manifest))?;

    if !plan.has_changes() {
        term.write_line(&format!(
            "{} No changes would be made",
            style("::").cyan().bold()
        ))?;
        return Ok(());
    }

    term.write_line("")?;
    print_plan(&term, &plan, verbose)?;

    term.write_line("")?;
    term.write_line(&format!(
        "{} Would apply {} change(s)",
        style("::").cyan().bold(),
        plan.change_count()
    ))?;

    Ok(())
}

fn cmd_status() -> Result<()> {
    let term = Term::stderr();
    let platform = sys_platform::Platform::detect()?;

    term.write_line(&format!(
        "{} sys.lua v{}",
        style("::").cyan().bold(),
        env!("CARGO_PKG_VERSION")
    ))?;
    term.write_line("")?;
    term.write_line(&format!("  Platform: {}", platform.platform))?;
    term.write_line(&format!("  OS:       {}", platform.os.as_str()))?;
    term.write_line(&format!("  Arch:     {}", platform.arch.as_str()))?;
    term.write_line(&format!("  User:     {}", platform.username))?;
    term.write_line(&format!("  Hostname: {}", platform.hostname))?;
    term.write_line(&format!("  Home:     {}", platform.home_dir.display()))?;
    term.write_line("")?;
    term.write_line(&format!(
        "  Store:    {}",
        platform.user_store_path().display()
    ))?;
    term.write_line(&format!("  Profile:  {}", platform.profile_dir().display()))?;

    Ok(())
}

/// Update inputs to their latest versions
fn cmd_update(config: &Path, input_name: Option<String>, verbose: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    term.write_line(&format!(
        "{} Evaluating {}",
        style("::").cyan().bold(),
        config.display()
    ))?;

    // Load manifest to get inputs
    let manifest = match Manifest::from_config(config) {
        Ok(m) => m,
        Err(e) => {
            term.write_line(&format!(
                "{} Failed to evaluate config: {}",
                style("error:").red().bold(),
                e
            ))?;
            std::process::exit(1);
        }
    };

    if manifest.inputs.is_empty() {
        term.write_line(&format!(
            "{} No inputs declared in config",
            style("::").cyan().bold()
        ))?;
        return Ok(());
    }

    // Set up input manager
    let cache_dir = platform.input_cache_dir();
    let lock_path = config
        .parent()
        .unwrap_or(Path::new("."))
        .join("syslua.lock");

    let mut manager = map_core_err(InputManager::new(cache_dir.clone(), lock_path.clone()))?;

    // Determine which inputs to update
    let inputs_to_update: Vec<_> = if let Some(ref name) = input_name {
        manifest
            .inputs
            .iter()
            .filter(|i| i.id == *name || i.source.contains(name))
            .collect()
    } else {
        manifest.inputs.iter().collect()
    };

    if inputs_to_update.is_empty() && input_name.is_some() {
        term.write_line(&format!(
            "{} Input '{}' not found in config",
            style("error:").red().bold(),
            input_name.as_ref().unwrap()
        ))?;
        std::process::exit(1);
    }

    term.write_line(&format!(
        "{} Updating {} input(s)",
        style("::").cyan().bold(),
        inputs_to_update.len()
    ))?;

    let mut updated = 0;
    let mut failed = 0;

    for input in inputs_to_update {
        let source = match InputSource::parse(&input.source) {
            Ok(s) => s,
            Err(e) => {
                term.write_line(&format!(
                    "  {} {} (invalid URI: {})",
                    style("✗").red().bold(),
                    input.source,
                    e
                ))?;
                failed += 1;
                continue;
            }
        };

        if verbose {
            term.write_line(&format!(
                "  {} Fetching {}",
                style("→").cyan().bold(),
                input.source
            ))?;
        }

        // Force update (update = true)
        match manager.resolve(&input.id, &source, true) {
            Ok(resolved) => {
                let revision_str = resolved
                    .revision
                    .as_ref()
                    .map(|r| format!(" ({})", &r[..8.min(r.len())]))
                    .unwrap_or_default();

                term.write_line(&format!(
                    "  {} {}{}",
                    style("✓").green().bold(),
                    input.source,
                    revision_str
                ))?;
                updated += 1;
            }
            Err(e) => {
                term.write_line(&format!(
                    "  {} {} ({})",
                    style("✗").red().bold(),
                    input.source,
                    e
                ))?;
                failed += 1;
            }
        }
    }

    // Save the lock file
    if updated > 0 {
        map_core_err(manager.save_lock_file())?;
    }

    term.write_line("")?;
    if failed > 0 {
        term.write_line(&format!(
            "{} Updated {} input(s), {} failed",
            style("::").yellow().bold(),
            updated,
            failed
        ))?;
    } else {
        term.write_line(&format!(
            "{} Updated {} input(s)",
            style("::").green().bold(),
            updated
        ))?;
    }

    if updated > 0 {
        term.write_line("")?;
        term.write_line(&format!(
            "Lock file updated: {}",
            style(lock_path.display()).cyan()
        ))?;
    }

    Ok(())
}

fn cmd_env(config: &Path, shell_name: Option<String>, print: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    // Check config exists
    if !config.exists() {
        term.write_line(&format!(
            "{} Config file not found: {}",
            style("error:").red().bold(),
            config.display()
        ))?;
        std::process::exit(1);
    }

    // Determine shell
    let shell = match shell_name {
        Some(name) => match name.to_lowercase().as_str() {
            "bash" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "fish" => Shell::Fish,
            "sh" => Shell::Sh,
            "powershell" | "pwsh" => Shell::PowerShell,
            _ => {
                term.write_line(&format!(
                    "{} Unknown shell: {}. Supported: bash, zsh, fish, sh, powershell",
                    style("error:").red().bold(),
                    name
                ))?;
                std::process::exit(1);
            }
        },
        None => Shell::detect(),
    };

    // Load manifest
    let manifest = match Manifest::from_config(config) {
        Ok(m) => m,
        Err(e) => {
            term.write_line(&format!(
                "{} Failed to evaluate config: {}",
                style("error:").red().bold(),
                e
            ))?;
            std::process::exit(1);
        }
    };

    if manifest.envs.is_empty() {
        term.write_line(&format!(
            "{} No environment variables declared in config",
            style("::").cyan().bold()
        ))?;
        return Ok(());
    }

    if print {
        // Print the script content
        let script = generate_env_script(&manifest, &shell);
        println!("{}", script);
    } else {
        // Write env scripts and print source command
        let env_dir = platform.env_script_dir();

        term.write_line(&format!(
            "{} Writing environment scripts to {}",
            style("::").cyan().bold(),
            env_dir.display()
        ))?;

        map_core_err(write_env_scripts(&manifest, &env_dir))?;

        // Print info about what was written
        term.write_line(&format!(
            "{} Generated scripts for {} env var(s)",
            style("::").green().bold(),
            manifest.envs.len()
        ))?;

        term.write_line("")?;
        term.write_line(&format!(
            "Add this to your shell config (~/.{}rc):",
            shell.as_str()
        ))?;
        term.write_line("")?;

        let cmd = source_command(&shell, &env_dir);
        println!("  {}", style(&cmd).cyan());

        term.write_line("")?;
        term.write_line("Or run it directly in the current shell:")?;
        term.write_line("")?;
        println!("  eval \"$(sys env --print)\"");
    }

    Ok(())
}

/// Show snapshot history
fn cmd_history(verbose: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    let snapshot_manager = SnapshotManager::new(platform.snapshots_dir());

    let snapshots = map_core_err(snapshot_manager.list_snapshots())?;
    let current_id = map_core_err(snapshot_manager.get_current_id())?;

    if snapshots.is_empty() {
        term.write_line(&format!("{} No snapshots found", style("::").cyan().bold()))?;
        term.write_line("")?;
        term.write_line("Run 'sys apply' to create your first snapshot.")?;
        return Ok(());
    }

    term.write_line(&format!(
        "{} Snapshot history ({} snapshots)",
        style("::").cyan().bold(),
        snapshots.len()
    ))?;
    term.write_line("")?;

    // Show snapshots in reverse chronological order (newest first)
    for (idx, snapshot) in snapshots.iter().rev().enumerate() {
        let is_current = current_id.as_ref() == Some(&snapshot.id);
        let marker = if is_current {
            style("*").green().bold()
        } else {
            style(" ").dim()
        };

        // Format timestamp
        let timestamp = format_timestamp(snapshot.created_at);

        // Truncate ID for display
        let short_id = if snapshot.id.len() > 13 {
            &snapshot.id[..13]
        } else {
            &snapshot.id
        };

        term.write_line(&format!(
            "  {} {} {} {}",
            marker,
            style(short_id).cyan(),
            style(&timestamp).dim(),
            snapshot.description
        ))?;

        if verbose {
            term.write_line(&format!(
                "      {} file(s), {} derivation(s)",
                snapshot.file_count, snapshot.derivation_count
            ))?;
        }

        // Only show the last 10 by default
        if idx >= 9 && !verbose {
            let remaining = snapshots.len() - 10;
            if remaining > 0 {
                term.write_line(&format!(
                    "  ... and {} more (use --verbose to see all)",
                    remaining
                ))?;
            }
            break;
        }
    }

    term.write_line("")?;
    if current_id.is_some() {
        term.write_line(&format!(
            "{} marks the current snapshot",
            style("*").green().bold()
        ))?;
    }
    term.write_line("Use 'sys rollback <id>' to restore a previous state.")?;

    Ok(())
}

/// Rollback to a previous snapshot
fn cmd_rollback(snapshot_id: Option<String>, skip_confirm: bool, verbose: bool) -> Result<()> {
    let term = Term::stderr();
    let platform = Platform::detect()?;

    let snapshot_manager = SnapshotManager::new(platform.snapshots_dir());

    // Determine target snapshot ID
    let target_id = match snapshot_id {
        Some(id) => id,
        None => {
            // Get the previous snapshot
            match map_core_err(snapshot_manager.get_previous_snapshot_id())? {
                Some(id) => {
                    term.write_line(&format!(
                        "{} No snapshot ID specified, using previous: {}",
                        style("::").cyan().bold(),
                        style(&id[..13.min(id.len())]).cyan()
                    ))?;
                    id
                }
                None => {
                    term.write_line(&format!(
                        "{} No previous snapshot to rollback to",
                        style("error:").red().bold()
                    ))?;
                    term.write_line("")?;
                    term.write_line("Run 'sys history' to see available snapshots.")?;
                    std::process::exit(1);
                }
            }
        }
    };

    // Get snapshot details
    let snapshot = map_core_err(snapshot_manager.get_snapshot(&target_id))?;

    // Show what will be rolled back
    term.write_line(&format!(
        "{} Rolling back to snapshot {}",
        style("::").cyan().bold(),
        style(&target_id[..13.min(target_id.len())]).cyan()
    ))?;
    term.write_line(&format!("  Description: {}", snapshot.description))?;
    term.write_line(&format!(
        "  Created:     {}",
        format_timestamp(snapshot.created_at)
    ))?;
    term.write_line(&format!("  Files:       {}", snapshot.files.len()))?;
    term.write_line(&format!("  Env vars:    {}", snapshot.envs.len()))?;

    if verbose {
        term.write_line("")?;
        term.write_line("Files to restore:")?;
        for file in &snapshot.files {
            term.write_line(&format!("  {} {}", style("→").cyan(), file.path.display()))?;
        }
    }

    // Confirm unless --yes is passed
    if !skip_confirm {
        term.write_line("")?;
        term.write_line(&format!(
            "{} This will restore your system to the state at this snapshot.",
            style("Warning:").yellow().bold()
        ))?;
        term.write_line(
            "Any files managed by sys.lua that were changed since then will be reverted.",
        )?;
        term.write_line("")?;

        use std::io::Write;
        print!("Continue? [y/N] ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            term.write_line("Rollback cancelled.")?;
            return Ok(());
        }
    }

    term.write_line("")?;
    term.write_line(&format!(
        "{} Performing rollback...",
        style("::").cyan().bold()
    ))?;

    // Perform the rollback
    let result = map_core_err(snapshot_manager.rollback_to(&target_id))?;

    // Report results
    if !result.files_restored.is_empty() && verbose {
        term.write_line("")?;
        term.write_line("Restored files:")?;
        for path in &result.files_restored {
            term.write_line(&format!("  {} {}", style("✓").green(), path.display()))?;
        }
    }

    if !result.files_removed.is_empty() && verbose {
        term.write_line("")?;
        term.write_line("Removed files:")?;
        for path in &result.files_removed {
            term.write_line(&format!("  {} {}", style("-").red(), path.display()))?;
        }
    }

    if !result.errors.is_empty() {
        term.write_line("")?;
        term.write_line(&format!(
            "{} Rollback completed with errors:",
            style("Warning:").yellow().bold()
        ))?;
        for error in &result.errors {
            term.write_line(&format!("  {} {}", style("✗").red(), error))?;
        }
    }

    term.write_line("")?;
    if result.is_success() {
        term.write_line(&format!(
            "{} Rollback complete: {} file(s) restored, {} file(s) removed",
            style("::").green().bold(),
            result.files_restored.len(),
            result.files_removed.len()
        ))?;
    } else {
        term.write_line(&format!(
            "{} Rollback completed with {} error(s)",
            style("::").yellow().bold(),
            result.errors.len()
        ))?;
    }

    Ok(())
}

/// Format a Unix timestamp as a human-readable string
fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    // Simple formatting without chrono crate
    if let Ok(elapsed) = datetime.elapsed() {
        let secs = elapsed.as_secs();
        if secs < 60 {
            "just now".to_string()
        } else if secs < 3600 {
            format!("{} min ago", secs / 60)
        } else if secs < 86400 {
            format!("{} hours ago", secs / 3600)
        } else if secs < 604800 {
            format!("{} days ago", secs / 86400)
        } else {
            format!("{} weeks ago", secs / 604800)
        }
    } else {
        // Future timestamp (shouldn't happen normally)
        "in the future".to_string()
    }
}

fn print_plan(term: &Term, plan: &Plan, verbose: bool) -> Result<()> {
    for change in plan.changes() {
        let symbol = match &change.kind {
            FileChangeKind::CreateSymlink { .. } | FileChangeKind::CreateContent { .. } => {
                style("+").green().bold()
            }
            FileChangeKind::UpdateSymlink { .. } | FileChangeKind::UpdateContent { .. } => {
                style("~").yellow().bold()
            }
            FileChangeKind::CopyFile { .. } => style("+").green().bold(),
            FileChangeKind::Unchanged => style(" ").dim(),
        };

        let description = change.description();

        term.write_line(&format!(
            "  {} {} {}",
            symbol,
            change.path.display(),
            style(format!("({})", description)).dim()
        ))?;

        // Show details in verbose mode
        if verbose {
            match &change.kind {
                FileChangeKind::CreateContent { content } => {
                    for line in content.lines().take(5) {
                        term.write_line(&format!("      {}", style(line).dim()))?;
                    }
                    let line_count = content.lines().count();
                    if line_count > 5 {
                        term.write_line(&format!(
                            "      {}",
                            style(format!("... ({} more lines)", line_count - 5)).dim()
                        ))?;
                    }
                }
                FileChangeKind::UpdateContent {
                    old_content,
                    new_content,
                } => {
                    term.write_line(&format!("      {}", style("--- old").red()))?;
                    for line in old_content.lines().take(3) {
                        term.write_line(&format!("      {}", style(format!("- {}", line)).red()))?;
                    }
                    term.write_line(&format!("      {}", style("+++ new").green()))?;
                    for line in new_content.lines().take(3) {
                        term.write_line(&format!(
                            "      {}",
                            style(format!("+ {}", line)).green()
                        ))?;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}
