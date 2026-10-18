// lena-cli/src/main.rs
//
// TODO: rename project to leña

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

mod callees;
mod callers;
mod doc;
mod index;
mod prompt;
mod query;
mod skeleton;

use anyhow::Result;
use clap::{Parser, Subcommand};
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
    /// Generate static HTML documentation from the indexed codebase
    Doc {
        /// Output directory for the static site
        #[arg(short, long, default_value = "dist")]
        out: PathBuf,
        /// Documentation theme: lena-book, mdbook, or single-page
        #[arg(short, long, default_value = "lena-book")]
        theme: String,
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
        include_source: Option<String>,
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
    /// Generate a call graph skeleton (indented, mermaid, or dot)
    Skeleton {
        /// The root Fully Qualified Symbol Name
        fqsn: String,
        /// Max depth to traverse
        #[arg(long, default_value = "1")]
        depth: usize,
        /// Output format: indented, mermaid, or dot
        #[arg(short, long, default_value = "indented")]
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

// TODO: remove all test templates (i.e. /my_template.j2)
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
        Commands::Doc {
            out,
            theme,
            css,
            github_actions,
        } => doc::run(&db, &out, &theme, css.as_ref(), github_actions).await?,
        Commands::Index {
            target,
            concurrency,
        } => index::run(target, concurrency, &db).await?,
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
            if let Some(i) = include_source {
                config.include_source = i;
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
        } => skeleton::run(&db, &fqsn, depth, &format).await?,
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
