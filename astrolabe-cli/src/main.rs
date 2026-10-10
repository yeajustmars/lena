// astrolabe-cli/src/main.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::Stdio, sync::Arc};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

use astrolabe_core::{
    db::AstrolabeDb,
    exporter::{ExportFormat, export_prompt},
    ingest::ingest_stream,
};

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
        /// The Fully Qualified Symbol Name (e.g. `rust:crates/astrolabe/src/main.rs::main`)
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
}

/// Helper to resolve adapter binary paths
fn resolve_binary(adapter_name: &str) -> String {
    let mut current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("astrolabe"));
    current_exe.pop();

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let adapter_filename = format!("{adapter_name}{exe_suffix}");
    let local_adapter_path = current_exe.join(&adapter_filename);

    if local_adapter_path.exists() {
        local_adapter_path.to_string_lossy().to_string()
    } else {
        adapter_filename
    }
}

/// Spawns a native directory-level adapter (Go, Rust, Clojure).
async fn run_native_adapter(
    adapter_name: &str,
    target: PathBuf,
    db: Arc<AstrolabeDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize> {
    let bin_to_run = resolve_binary(adapter_name);
    info!(
        "Starting native adapter [{adapter_name}] on {}",
        target.display()
    );

    let mut child = Command::new(&bin_to_run)
        .arg(&target)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to spawn adapter binary: {bin_to_run}"))?;

    let mut facts_ingested = 0;
    if let Some(stdout) = child.stdout.take() {
        facts_ingested = ingest_stream(&db, stdout).await?;
    }

    let status = child.wait().await?;
    if status.success() {
        info!("Adapter [{adapter_name}] finished. Ingested {facts_ingested} facts.");
    } else {
        warn!("Adapter [{adapter_name}] exited with non-zero status: {status}");
    }

    Ok(facts_ingested)
}

async fn run_treesitter_directory(
    target: PathBuf,
    lang: &str,
    grammar_file: PathBuf,
    db: Arc<AstrolabeDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize> {
    let bin_to_run = resolve_binary("astrolabe-adapter-treesitter");

    let mut child = Command::new(&bin_to_run)
        .arg(&target)
        .arg("--lang")
        .arg(lang)
        .arg("--grammar")
        .arg(&grammar_file)
        // Notice we omitted --query, letting the adapter use its embedded defaults
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to spawn {bin_to_run} for {}", target.display()))?;

    let mut facts_ingested = 0;
    if let Some(stdout) = child.stdout.take() {
        facts_ingested = ingest_stream(&db, stdout).await?;
    }

    let status = child.wait().await?;
    if status.success() {
        info!("Tree-sitter adapter [{lang}] finished. Ingested {facts_ingested} facts.");
    } else {
        warn!("Tree-sitter adapter failed for language {}", lang);
    }

    Ok(facts_ingested)
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
        Commands::Callers { fqsn } => {
            let query = format!(
                "[:find ?caller_fqsn
                  :where [?callee :symbol/fqsn \"{fqsn}\"]
                         (calls_rule ?caller ?callee)
                         [?caller :symbol/fqsn ?caller_fqsn]]"
            );
            match db.query_with_rules(&query).await {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("Query error: {e}"),
            }
        }
        Commands::Callees { fqsn } => {
            let query = format!(
                "[:find ?callee_fqsn
                  :where [?caller :symbol/fqsn \"{fqsn}\"]
                         (calls_rule ?caller ?callee)
                         [?callee :symbol/fqsn ?callee_fqsn]]"
            );
            match db.query_with_rules(&query).await {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("Query error: {e}"),
            }
        }
        Commands::Index {
            target,
            queries_dir: _,
            grammars_dir,
            concurrency,
        } => {
            info!(
                "Beginning concurrent Astrolabe indexing on: {}",
                target.display()
            );

            let mut join_set = JoinSet::new();
            let semaphore = Arc::new(Semaphore::new(concurrency));

            // 1. Dispatch Native Adapters
            let native_adapters = vec![
                "astrolabe-adapter-clj",
                "astrolabe-adapter-go",
                "astrolabe-adapter-rust",
            ];
            for adapter in native_adapters {
                let permit = semaphore.clone().acquire_owned().await.unwrap();
                let db_clone = Arc::clone(&db);
                let target_clone = target.clone();
                join_set.spawn(run_native_adapter(adapter, target_clone, db_clone, permit));
            }

            let dylib_ext = std::env::consts::DLL_EXTENSION;
            let ts_languages = vec![
                ("python", format!("tree_sitter_python.{dylib_ext}")),
                ("javascript", format!("tree_sitter_javascript.{dylib_ext}")),
            ];

            for (lang, grammar_name) in ts_languages {
                let grammar_path = grammars_dir.join(grammar_name);
                if grammar_path.exists() {
                    let permit = semaphore.clone().acquire_owned().await.unwrap();
                    let db_clone = Arc::clone(&db);
                    let target_clone = target.clone();
                    join_set.spawn(run_treesitter_directory(
                        target_clone,
                        lang,
                        grammar_path,
                        db_clone,
                        permit,
                    ));
                } else {
                    warn!(
                        "Skipping {} indexing: Grammar file {:?} not found.",
                        lang, grammar_path
                    );
                }
            }

            // 3. Await all tasks and aggregate totals
            let mut total_facts = 0;
            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok(Ok(facts)) => total_facts += facts,
                    Ok(Err(e)) => error!("Adapter task failed: {e}"),
                    Err(e) => error!("Task join error: {e}"),
                }
            }

            info!("Indexing complete. Total facts ingested across all languages: {total_facts}");
        }
        Commands::Prompt { fqsn, format } => {
            let export_format = match format.to_lowercase().as_str() {
                "md" | "markdown" => ExportFormat::Markdown,
                "xml" => ExportFormat::Xml,
                _ => {
                    eprintln!("Invalid format. Use 'xml' or 'md'.");
                    return Ok(());
                }
            };

            match export_prompt(&db, &fqsn, export_format).await {
                Ok(output) => println!("{output}"),
                Err(e) => eprintln!("Failed to export prompt: {e}"),
            }
        }
        Commands::Query { query } => match db.query_with_rules(&query).await {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Query error: {e}"),
        },
    }

    Ok(())
}
