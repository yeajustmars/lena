// lena-cli/src/index.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use lena_core::{db::LenaDb, ingest::ingest_stream};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};
use tracing::{error, info, warn};

fn resolve_binary(adapter_name: &str) -> String {
    let mut current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lena"));
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

async fn run_native_adapter(
    adapter_name: &str,
    target: PathBuf,
    db: Arc<LenaDb>,
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
    db: Arc<LenaDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize> {
    let bin_to_run = resolve_binary("lena-adapter-treesitter");

    let mut child = Command::new(&bin_to_run)
        .arg(&target)
        .arg("--lang")
        .arg(lang)
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
        warn!("Tree-sitter adapter failed for language {lang}");
    }

    Ok(facts_ingested)
}

pub async fn run(target: PathBuf, concurrency: usize, db: &Arc<LenaDb>) -> Result<()> {
    info!(
        "Beginning concurrent Lena indexing on: {}",
        target.display()
    );

    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let native_adapters = ["lena-adapter-clj", "lena-adapter-go", "lena-adapter-rust"];
    for adapter in native_adapters {
        let permit = Arc::clone(&semaphore).acquire_owned().await?;
        let db_clone = Arc::clone(db);
        let target_clone = target.clone();
        join_set.spawn(run_native_adapter(adapter, target_clone, db_clone, permit));
    }

    let ts_languages = ["python", "javascript"];

    for lang in ts_languages {
        let permit = Arc::clone(&semaphore).acquire_owned().await?;
        let db_clone = Arc::clone(db);
        let target_clone = target.clone();

        join_set.spawn(run_treesitter_directory(
            target_clone,
            lang,
            db_clone,
            permit,
        ));
    }

    let mut total_facts = 0;
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(facts)) => total_facts += facts,
            Ok(Err(e)) => error!("Adapter task failed: {e}"),
            Err(e) => error!("Task join error: {e}"),
        }
    }

    info!("Indexing complete. Total facts ingested across all languages: {total_facts}");
    Ok(())
}
