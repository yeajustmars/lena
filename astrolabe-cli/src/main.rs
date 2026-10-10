// astrolabe-cli/src/main.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

mod callees;
mod callers;
mod index;
mod prompt;
mod query;

use anyhow::Result;
use astrolabe_core::db::AstrolabeDb;
use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc};
use tracing::Level;
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

        /// Path to the directory containing Tree-sitter .scm query files
        #[arg(long, default_value = "queries")]
        queries_dir: PathBuf,

        /// Path to the directory containing Tree-sitter grammar binaries (.so, .dylib, .dll)
        #[arg(long, default_value = "grammars")]
        grammars_dir: PathBuf,

        /// Maximum number of concurrent subprocesses (default: 16)
        #[arg(long, default_value = "16")]
        concurrency: usize,
    },
    /// Export an entity as LLM Context
    Prompt {
        /// The Fully Qualified Symbol Name
        fqsn: String,
        /// Format to export (xml or md)
        #[arg(short, long, default_value = "xml")]
        format: String,
    },
    /// Query the graph using Datalog
    Query {
        /// Datalog query string
        query: String,
    },
    /// Show all functions that call the given symbol (transitively)
    Callers {
        /// The Fully Qualified Symbol Name
        fqsn: String,
    },
    /// Show all functions called by the given symbol (transitively)
    Callees {
        /// The Fully Qualified Symbol Name
        fqsn: String,
    },
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

    if let Some(parent) = cli.db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let db = Arc::new(AstrolabeDb::open(&cli.db_path)?);

    match cli.command {
        Commands::Index {
            target,
            queries_dir: _,
            grammars_dir,
            concurrency,
        } => index::run(target, &grammars_dir, concurrency, &db).await?,
        Commands::Prompt { fqsn, format } => prompt::run(&db, &fqsn, &format).await?,
        Commands::Query { query: q } => query::run(&db, &q).await?,
        Commands::Callers { fqsn } => callers::run(&db, &fqsn).await?,
        Commands::Callees { fqsn } => callees::run(&db, &fqsn).await?,
    }

    Ok(())
}
