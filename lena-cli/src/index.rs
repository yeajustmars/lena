// lena-cli/src/index.rs

//#![deny(warnings)]
//#![warn(clippy::all, clippy::pedantic)]
//TODO: turn on clippy::pedantic

use anyhow::{Context, Result, bail};
use lena_core::{
    config::{AdapterDefinition, AdaptersConfig},
    db::LenaDb,
    ingest::ingest_stream,
};
use std::{path::PathBuf, process::Stdio, sync::Arc};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};
use tracing::{debug, error, info, warn};

fn resolve_binary(command_name: &str) -> Option<PathBuf> {
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let adapter_filename = format!("{command_name}{exe_suffix}");

    if let Some(home) = dirs::home_dir() {
        let lena_bin = home.join(".lena").join("bin").join(&adapter_filename);
        if lena_bin.exists() {
            return Some(lena_bin);
        }
    }

    let mut current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lena"));
    current_exe.pop();
    let local_adapter_path = current_exe.join(&adapter_filename);
    if local_adapter_path.exists() {
        return Some(local_adapter_path);
    }

    which::which(&adapter_filename).ok()
}

fn expand_tilde(path_str: &str) -> String {
    if path_str.starts_with("~/")
        && let Some(home) = dirs::home_dir()
    {
        return path_str.replacen('~', &home.to_string_lossy(), 1);
    }
    path_str.to_string()
}

async fn run_adapter(
    lang: String,
    def: AdapterDefinition,
    target: PathBuf,
    db: Arc<LenaDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize> {
    let Some(bin_path) = resolve_binary(&def.command) else {
        bail!(
            "Adapter executable '{}' not found in ~/.lena/bin or PATH.",
            def.command
        );
    };

    let mut expanded_args = Vec::new();
    for arg in def.args {
        let expanded = expand_tilde(&arg);
        if expanded.ends_with(".so") && !PathBuf::from(&expanded).exists() {
            bail!("Required grammar file not found: {expanded}");
        }
        expanded_args.push(expanded);
    }

    info!("Starting adapter [{lang}] via {}", def.command);

    let mut child = Command::new(&bin_path)
        .args(&expanded_args)
        .arg(&target)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to spawn {}", bin_path.display()))?;

    let mut facts_ingested = 0;
    if let Some(stdout) = child.stdout.take() {
        facts_ingested = ingest_stream(&db, stdout).await?;
    }

    let status = child.wait().await?;
    if status.success() {
        info!("Adapter [{lang}] finished. Ingested {facts_ingested} facts.");
    } else {
        warn!("Adapter [{lang}] exited with non-zero status: {status}");
    }

    Ok(facts_ingested)
}

pub async fn run(
    target: PathBuf,
    concurrency: usize,
    db: &Arc<LenaDb>,
    explicit_adapters: Option<Vec<String>>,
    run_all: bool,
    config: AdaptersConfig,
) -> Result<()> {
    let mut targets_to_run = Vec::new();

    if run_all {
        for (lang, def) in config.registered {
            if resolve_binary(&def.command).is_some() {
                // If it requires a grammar, only auto-run if the grammar exists
                let has_missing_grammar = def.args.iter().any(|a| {
                    let exp = expand_tilde(a);
                    exp.ends_with(".so") && !PathBuf::from(exp).exists()
                });

                if has_missing_grammar {
                    debug!("Skipping [{lang}] due to missing .so grammar file.");
                } else {
                    targets_to_run.push((lang, def));
                }
            } else {
                debug!(
                    "Skipping [{lang}] because executable '{}' is not installed.",
                    def.command
                );
            }
        }
    } else if let Some(adapters) = explicit_adapters {
        for lang in adapters {
            if let Some(def) = config.registered.get(&lang) {
                targets_to_run.push((lang, def.clone()));
            } else {
                bail!(
                    "Unknown adapter '{}'. Check your lena.toml configuration.",
                    lang
                );
            }
        }
    }

    if targets_to_run.is_empty() {
        info!("No adapters executed. Ensure adapters are installed or check your filters.");
        return Ok(());
    }

    info!(
        "Beginning concurrent Leña indexing on: {}",
        target.display()
    );

    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(concurrency));

    for (lang, def) in targets_to_run {
        let permit = Arc::clone(&semaphore).acquire_owned().await?;
        let db_clone = Arc::clone(db);
        let target_clone = target.clone();

        join_set.spawn(run_adapter(lang, def, target_clone, db_clone, permit));
    }

    let mut total_facts = 0;
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(facts)) => total_facts += facts,
            Ok(Err(e)) => error!("Adapter task failed: {e}"),
            Err(e) => error!("Task join error: {e}"),
        }
    }

    info!("Indexing complete. Total facts ingested: {total_facts}");
    Ok(())
}
