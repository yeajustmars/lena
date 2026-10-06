#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use astrolabe_core::db::AstrolabeDb;
use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc};
use tracing::{Level, info};
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    info!("Opening database at {}", cli.db_path.display());
    let db = Arc::new(AstrolabeDb::open(&cli.db_path)?);

    match cli.command {
        Commands::Index { target } => {
            info!("Indexing target: {}", target.display());
            // TODO: In Phase 3, we will spawn the adapters here, pipe their stdout,
            // and pass the handle to `astrolabe_core::ingest::ingest_stream`.
            println!("Index command ready to integrate adapters.");
        }
        Commands::Query { query } => {
            info!("Executing query...");
            match db.inner().query_json(&query).await {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("Query error: {e}"),
            }
        }
    }

    Ok(())
}
