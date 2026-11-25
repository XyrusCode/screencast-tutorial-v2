use anyhow::{bail, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "screencast-v2",
    version,
    about = "Narrated screen-walkthroughs driven by Vercel agent-browser",
    long_about = None
)]
struct Cli {
    /// Path to the app config JSON (used by live stages)
    #[arg(long, global = true, default_value = "config/app.config.json")]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Write starter config and script files into a directory
    Init {
        /// Destination directory
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Inspect app routes and dump observed selectors
    Inspect {
        /// Comma-separated routes to inspect
        #[arg(long, default_value = "/")]
        routes: String,
        /// Output JSON path (defaults to <output_dir>/ui-inspection.json)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Expand navigation menus before listing routes
        #[arg(long)]
        discover: bool,
    },
    /// Validate a script against the live app
    Validate {
        #[arg(long)]
        script: PathBuf,
    },
    /// Generate and cache narration audio
    Audio {
        #[arg(long)]
        script: PathBuf,
    },
    /// Record a continuous browser take with timing markers
    Record {
        #[arg(long)]
        script: PathBuf,
    },
    /// Cut, sync and mux a recorded take into final videos
    Merge {
        #[arg(long)]
        script: PathBuf,
    },
    /// Run audio, record and merge in one go
    All {
        #[arg(long)]
        script: PathBuf,
    },
}

fn main() -> Result<()> {
    let Cli { config, command } = Cli::parse();

    match command {
        Some(command) => run(command, &config),
        None => {
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

fn run(command: Commands, config_path: &Path) -> Result<()> {
    match command {
        Commands::Init { dir } => init(&dir),
        Commands::Inspect {
            routes,
            out,
            discover,
        } => {
            let config = screencast_v2::config::load_config(config_path)?;
            let out = out
                .unwrap_or_else(|| Path::new(&config.output_dir).join("ui-inspection.json"));
            screencast_v2::inspect::run(&config, &routes, &out, discover)
        }
        Commands::Validate { script } => {
            let config = screencast_v2::config::load_config(config_path)?;
            screencast_v2::validate::run(&config, &script)
        }
        Commands::Audio { .. } => not_yet("audio"),
        Commands::Record { .. } => not_yet("record"),
        Commands::Merge { .. } => not_yet("merge"),
        Commands::All { .. } => not_yet("all"),
    }
}

fn init(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir.join("config"))?;

    let config_path = dir.join("config/app.config.json");
    let script_path = dir.join("config/script.example.json");

    if !config_path.exists() {
        fs::write(&config_path, include_str!("../assets/config.example.json"))?;
    }
    if !script_path.exists() {
        fs::write(&script_path, include_str!("../assets/script.example.json"))?;
    }

    println!("starter config: {}", config_path.display());
    println!("starter script: {}", script_path.display());
    Ok(())
}

fn not_yet(stage: &str) -> Result<()> {
    bail!("stage `{stage}` is not implemented in 0.1.0 — see references/architecture.md for the rollout order")
}
