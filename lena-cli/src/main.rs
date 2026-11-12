// lena-cli/src/main.rs
//
// TODO: rename project to leña

// #![deny(warnings)]
// #![warn(clippy::all, clippy::pedantic)]
// TODO: reinstatiate clippy::pendantic

mod adapter;
mod callees;
mod callers;
mod doc;
mod index;
mod list;
mod prompt;
mod query;
mod skeleton;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use lena_core::db::LenaDb;
use std::{path::PathBuf, sync::Arc};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "lena")]
#[command(author, version, about = "Universal Codebase AST/IR Graph Engine", long_about = None)]
struct Cli {
    /// Turn on verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Path to the Lena database directory (defaults to .lena/db in current dir)
    #[arg(long, global = true, default_value = ".lena/db")]
    db_path: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage language adapters (list, install)
    Adapter {
        /// Action to perform: 'list' or 'install'
        action: String,
        /// Adapter name to install (e.g., 'rust', 'go')
        name: Option<String>,
    },
    /// Show all functions called by the given symbol (transitively)
    Callees {
        /// The Fully Qualified Symbol Name
        fqsn: String,
    },
    /// Show all functions that call the given symbol (transitively)
    Callers {
        /// The Fully Qualified Symbol Name
        fqsn: String,
    },
    /// Validate adapters, grammars, and directory support without running a full index
    Check {
        /// Target directory to check
        #[arg(default_value = ".")]
        target: PathBuf,
    },
    /// Generate shell completion scripts
    Completion {
        /// The shell to generate the script for (bash, elvish, fish, powershell, zsh)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Generate static HTML documentation from the indexed codebase
    Doc {
        /// Output directory for the static site
        #[arg(short, long, default_value = "docs")]
        out: PathBuf,
        /// Documentation format: book, single-page, or mdbook
        #[arg(short, long, default_value = "book")]
        format: String,
        /// Optional path to custom CSS file to override default styles
        #[arg(long)]
        css: Option<PathBuf>,
        /// Generate a GitHub Pages deployment workflow
        #[arg(long)]
        github_actions: bool,
    },
    /// Index a codebase by running available adapters
    Index {
        /// Target directory to index
        #[arg(default_value = ".")]
        target: PathBuf,

        /// Maximum number of concurrent subprocesses (default: 16)
        #[arg(long, default_value = "16")]
        concurrency: usize,

        /// Specific adapter(s) to run (e.g., -a rust -a python)
        #[arg(short = 'a', long = "adapter", required_unless_present = "all")]
        adapters: Option<Vec<String>>,

        /// Run all installed adapters registered in lena.toml
        #[arg(short = 'A', long, conflicts_with = "adapters")]
        all: bool,

        /// Run the full adapter and validation pipeline without writing to the database
        #[arg(long)]
        dry_run: bool,
    },
    /// List available Fully Qualified Symbol Names (FQSNs) in the database
    List {
        /// Optional regex pattern to filter the symbol list
        #[arg(short, long)]
        filter: Option<String>,
        /// List all symbols in the database (warning: output can be very large)
        #[arg(short, long)]
        all: bool,
    },
    /// Export an entity as LLM Context
    Prompt {
        /// The Fully Qualified Symbol Name
        fqsn: String,
        /// Format to export (xml, md, or template)
        #[arg(short, long, default_value = "xml")]
        format: String,
        /// Path to custom Jinja2 template file (required if format is 'template')
        #[arg(short, long)]
        template: Option<PathBuf>,
        /// Override prompt traversal depth
        #[arg(long)]
        depth: Option<usize>,
        /// Determines which symbols get full source (e.g., "root", "all", "none")
        #[arg(long)]
        include_source: Option<bool>,
        /// Regex patterns matching symbol names to include full source
        #[arg(long)]
        source_pattern: Option<Vec<String>>,
        /// Namespace prefixes to include full source
        #[arg(long)]
        source_ns: Option<Vec<String>>,
        /// Omit docstrings from the prompt context
        #[arg(long)]
        no_docs: bool,
        /// Omit call-site linkages from the prompt context
        #[arg(long)]
        no_call_sites: bool,
        /// Hard token limit guardrail
        #[arg(long)]
        max_tokens: Option<usize>,
    },
    /// Query the graph using Datalog
    Query {
        /// Datalog query string
        query: String,
    },
    /// Generate a hierarchical call-graph skeleton for a symbol
    Skeleton {
        /// Fully Qualified Symbol Name (e.g., rust:crate::module::func)
        fqsn: String,
        /// Maximum traversal depth (default: 3)
        #[arg(short, long, default_value_t = 3)]
        depth: usize,
        /// Output format: indented (default), mermaid, or dot
        #[arg(long, default_value = "indented")]
        format: String,
        /// Invert traversal: show who calls this symbol (upstream) rather than who it calls
        #[arg(long)]
        by_callee: bool,
    },
    /// Export an entire directory of source code into a single context file
    Snapshot {
        /// The path to the directory (or file) to export
        path: PathBuf,
        /// The output format: markdown or xml
        #[arg(short, long, default_value = "markdown")]
        format: String,
        /// Optional path to save the output file directly
        #[arg(short, long)]
        out: Option<PathBuf>,
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

    let db = Arc::new(LenaDb::open(&cli.db_path)?);

    match cli.command {
        Commands::List { filter, all } => list::run(&db, filter.as_deref(), all).await?,
        Commands::Adapter { action, name } => adapter::run(&action, name.as_deref()).await?,
        Commands::Check { target } => {
            // Stub for Check pre-flight logic (to be implemented in Phase 4 if needed)
            println!(
                "Validating Leña configuration and adapters for target: {}",
                target.display()
            );
            let config =
                lena_core::config::LenaConfig::load(std::env::current_dir().ok().as_deref())?;
            println!("Configuration loaded successfully.");
            println!(
                "Total registered adapters: {}",
                config.adapters.registered.len()
            );
            // TODO: implement detailed pre-flight checks here.
        }
        // TODO: move to completion.rs
        Commands::Completion { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
        Commands::Doc {
            out,
            format,
            css,
            github_actions,
        } => doc::run(&db, &out, &format, css.as_ref(), github_actions).await?,
        Commands::Index {
            target,
            concurrency,
            adapters,
            all,
            dry_run,
        } => {
            let config =
                lena_core::config::LenaConfig::load(std::env::current_dir().ok().as_deref())?;
            index::run(
                target,
                concurrency,
                &db,
                adapters,
                all,
                dry_run,
                config.adapters,
                config.report,
            )
            .await?;
        }
        Commands::Prompt {
            fqsn,
            format,
            template,
            depth,
            include_source,
            source_pattern,
            source_ns,
            no_docs,
            no_call_sites,
            max_tokens,
        } => {
            let mut config =
                lena_core::config::LenaConfig::load(std::env::current_dir().ok().as_deref())?
                    .prompt;

            // Apply CLI flag overrides
            if let Some(d) = depth {
                config.depth = d;
            }

            if include_source.unwrap_or(false) {
                config.include_source = true;
            }
            if let Some(p) = source_pattern {
                config.source_patterns = p;
            }
            if let Some(ns) = source_ns {
                config.source_namespaces = ns;
            }
            if no_docs {
                config.include_docs = false;
            }
            if no_call_sites {
                config.call_sites = false;
            }
            if let Some(m) = max_tokens {
                config.max_tokens = Some(m);
            }

            prompt::run(&db, &fqsn, &format, template.as_ref(), config).await?
        }
        Commands::Skeleton {
            fqsn,
            depth,
            format,
            by_callee,
        } => skeleton::run(&db, &fqsn, depth, &format, by_callee).await?,
        Commands::Query { query: q } => query::run(&db, &q).await?,
        Commands::Callers { fqsn } => callers::run(&db, &fqsn).await?,
        Commands::Callees { fqsn } => callees::run(&db, &fqsn).await?,
        // TODO: move entire logic to exporter.rs file
        Commands::Snapshot { path, format, out } => {
            use lena_core::exporter::{ExportFormat, export_directory};

            let export_fmt = match format.to_lowercase().as_str() {
                "xml" => ExportFormat::Xml,
                "markdown" | "md" => ExportFormat::Markdown,
                _ => {
                    eprintln!("Invalid format. Use 'markdown' or 'xml'.");
                    std::process::exit(1);
                }
            };

            match export_directory(&path, export_fmt) {
                Ok(content) => {
                    if let Some(out_path) = out {
                        std::fs::write(&out_path, content)?;
                        println!("Snapshot successfully exported to {}", out_path.display());
                    } else {
                        println!("{content}");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to generate snapshot: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
