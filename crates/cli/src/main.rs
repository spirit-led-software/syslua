mod cmd;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cmd::{cmd_apply, cmd_destroy, cmd_info, cmd_plan};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "syslua", author, version, about, long_about = None)]
struct Cli {
  #[arg(short, long, global = true)]
  verbose: bool,

  #[command(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  /// Evaluate a config and apply changes to the system
  Apply { file: String },
  /// Evaluate a config and create a plan without applying
  Plan { file: String },
  /// Remove resources defined in a config
  Destroy { file: String },
  /// Display system information
  Info,
}

fn main() -> ExitCode {
  let cli = Cli::parse();

  let level = if cli.verbose { Level::DEBUG } else { Level::INFO };

  FmtSubscriber::builder()
    .with_max_level(level)
    .with_target(false)
    .without_time()
    .init();

  let result = match cli.command {
    Commands::Apply { file } => {
      cmd_apply(&file);
      Ok(())
    }
    Commands::Plan { file } => cmd_plan(&file),
    Commands::Destroy { file } => {
      cmd_destroy(&file);
      Ok(())
    }
    Commands::Info => {
      cmd_info();
      Ok(())
    }
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      eprintln!("Error: {err:?}");
      ExitCode::FAILURE
    }
  }
}
