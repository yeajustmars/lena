// crates/astrolabe-cli/src/main.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use astrolabe_core::{db::AstrolabeDb, ingest::ingest_stream};
use clap::{Parser, Subcommand};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
use tokio::process::Command;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "astrolabe")]
#[command(author, version, about = "Universal Codebase AST/IR Graph Engine", long_about = None)]
struct Cli {
    /// Turn on verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Path to the Astrolabe database directory (defaults to .astrolabe/db in current dir)
    #[arg(long, global = true, default_value = ".astrolabe/db")]
    db_path: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a codebase by running available adapters
    Index {
        /// Target directory to index
        #[arg(default_value = ".")]
        target: PathBuf,
    },
    /// Query the graph using Datalog
    Query {
        /// Datalog query string
        query: String,
    },
}

/// Spawns an adapter, hooks up its standard output to our ingestion stream, and waits for it to finish.
async fn run_adapter(adapter_name: &str, target: &Path, db: &Arc<AstrolabeDb>) -> Result<()> {
    // 1. Try to find the adapter next to our own executable (e.g., target/debug/)
    let mut current_exe = std::env::current_exe()?;
    current_exe.pop(); // Remove the "astrolabe" binary name from the path

    // Add .exe for Windows compatibility if needed
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let adapter_filename = format!("{adapter_name}{exe_suffix}");
    let local_adapter_path = current_exe.join(&adapter_filename);

    let bin_to_run = if local_adapter_path.exists() {
        local_adapter_path.to_string_lossy().to_string()
    } else {
        // 2. Fallback to system $PATH
        adapter_filename
    };

    info!("Starting adapter [{adapter_name}] on {}", target.display());

    let mut child = Command::new(&bin_to_run)
        .arg(target)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // Let adapter errors pass through to the terminal
        .spawn()
        .with_context(|| format!("Failed to spawn adapter binary: {bin_to_run}"))?;

    // 3. Take stdout and stream it directly into MesoDB
    if let Some(stdout) = child.stdout.take() {
        let facts_ingested = ingest_stream(db, stdout).await?;
        info!("Adapter [{adapter_name}] finished. Ingested {facts_ingested} facts.");
    }

    let status = child.wait().await?;
    if !status.success() {
        warn!("Adapter [{adapter_name}] exited with non-zero status: {status}");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");

    // Ensure parent directories exist
    if let Some(parent) = cli.db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let db = Arc::new(AstrolabeDb::open(&cli.db_path)?);

    match cli.command {
        Commands::Index { target } => {
            info!("Beginning Astrolabe indexing on: {}", target.display());

            // Sequentially run our two primary adapters.
            // In the future, this can dynamically discover adapters in the ~/.config/astrolabe/adapters dir.

            if let Err(e) = run_adapter("astrolabe-adapter-rust", &target, &db).await {
                error!("Rust adapter error: {e}");
            }

            if let Err(e) = run_adapter("astrolabe-adapter-clj", &target, &db).await {
                error!("Clojure adapter error: {e}");
            }

            info!("Indexing complete.");
        }
        Commands::Query { query } => match db.inner().query_json(&query).await {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Query error: {e}"),
        },
    }

    Ok(())
}
