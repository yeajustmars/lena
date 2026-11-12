// lena-cli/src/index.rs

#![deny(warnings)]
// #![warn(clippy::all, clippy::pedantic)]
// TODO: turn pedantic back on and lint

use anyhow::{Context, Result, bail};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lena_core::{
    config::{AdapterDefinition, AdaptersConfig},
    db::LenaDb,
    ingest::ingest_stream,
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Instant,
};
use tokio::{process::Command, sync::Semaphore, task::JoinSet};
use tracing::{debug, error, info, warn};

fn resolve_binary(command_name: &str) -> Option<PathBuf> {
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let adapter_filename = format!("{command_name}{exe_suffix}");

    // 1. Local workspace (if running inside the Leña repo)
    let workspace_bin = PathBuf::from("target")
        .join("debug")
        .join(&adapter_filename);
    if Path::new("Cargo.toml").exists() && workspace_bin.exists() {
        return Some(workspace_bin);
    }

    // 2. ~/.lena/bin/ (Managed installs)
    if let Some(home) = dirs::home_dir() {
        let managed_bin = home.join(".lena").join("bin").join(&adapter_filename);
        if managed_bin.exists() {
            return Some(managed_bin);
        }
    }

    // 3. Sibling directory (e.g., if running from a globally installed target/release folder)
    let mut current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lena"));
    current_exe.pop();
    let sibling_path = current_exe.join(&adapter_filename);
    if sibling_path.exists() {
        return Some(sibling_path);
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
    pb: ProgressBar,
) -> Result<usize> {
    let adapter_start = Instant::now();
    let Some(bin_path) = resolve_binary(&def.command) else {
        pb.finish_with_message(format!("❌ [{lang}] executable not found"));
        bail!(
            "Adapter executable '{}' not found in ~/.lena/bin or PATH.",
            def.command
        );
    };

    let mut expanded_args = Vec::new();
    for arg in def.args {
        expanded_args.push(expand_tilde(&arg));
    }

    pb.set_message(format!("[{lang}] indexing..."));

    let mut child = Command::new(&bin_path)
        .args(&expanded_args)
        .arg(&target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // Capture stderr for sidecar protocols
        .spawn()
        .with_context(|| format!("Failed to spawn {}", bin_path.display()))?;

    let mut facts_ingested = 0;

    // Concurrently process stderr for @PROG: and @DIAG:
    if let Some(stderr) = child.stderr.take() {
        let pb_clone = pb.clone();
        let lang_clone = lang.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut reader = BufReader::new(stderr).lines();
            // TODO: clean up all this nesting
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(prog_str) = line.strip_prefix("@PROG:") {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(prog_str) {
                        if v["status"] == "init" {
                            if let Some(total) = v["total"].as_u64() {
                                pb_clone.set_length(total);
                                pb_clone.set_style(
                                    indicatif::ProgressStyle::with_template(
                                        "{spinner:.green} [{bar:20.cyan/blue}] {pos}/{len} {msg}",
                                    )
                                    .unwrap()
                                    .progress_chars("#>-"),
                                );
                            }
                        } else if v["status"] == "parsing" {
                            pb_clone.inc(1);
                            if let Some(f) = v["file"].as_str() {
                                pb_clone.set_message(format!("parsing {f}"));
                            }
                        }
                    }
                } else if let Some(diag_str) = line.strip_prefix("@DIAG:") {
                    // Temporarily suspend the progress bar to log the diagnostic cleanly
                    pb_clone.suspend(|| {
                        tracing::warn!("[{lang_clone} DIAG] {diag_str}");
                    });
                } else {
                    pb_clone.suspend(|| {
                        tracing::debug!("[{lang_clone}] {line}");
                    });
                }
            }
        });
    }

    if let Some(stdout) = child.stdout.take() {
        facts_ingested = ingest_stream(&db, stdout, false, &lang).await?;
    }

    let status = child.wait().await?;
    let elapsed = adapter_start.elapsed();

    if status.success() {
        if facts_ingested > 0 {
            pb.finish_with_message(format!(
                "✅ [{lang}] done: {facts_ingested} facts ({elapsed:.2?})"
            ));
        } else {
            pb.finish_with_message(format!("✅ [{lang}] done: 0 facts ({elapsed:.2?})"));
        }
    } else {
        pb.finish_with_message(format!("⚠️ [{lang}] exited with {status}"));
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
    let start_time = Instant::now();
    let mut targets_to_run = Vec::new();

    if run_all {
        for (lang, def) in config.registered {
            if resolve_binary(&def.command).is_some() {
                targets_to_run.push((lang, def));
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
                bail!("Unknown adapter '{lang}'. Check your lena.toml configuration.");
            }
        }
    }

    if targets_to_run.is_empty() {
        info!("No adapters executed. Ensure adapters are installed or check your filters.");
        return Ok(());
    }

    let m = MultiProgress::new();
    let spinner_style = ProgressStyle::with_template("{spinner:.green} {msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

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

        let pb = m.add(ProgressBar::new_spinner());
        pb.set_style(spinner_style.clone());
        pb.set_message(format!("[{lang}] starting..."));
        pb.enable_steady_tick(std::time::Duration::from_millis(120));

        join_set.spawn(run_adapter(lang, def, target_clone, db_clone, permit, pb));
    }

    let mut total_facts = 0;
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(facts)) => total_facts += facts,
            Ok(Err(e)) => error!("Adapter task failed: {e}"),
            Err(e) => error!("Task join error: {e}"),
        }
    }

    m.clear().ok(); // Clear the spinners to print the final summary cleanly
    let elapsed = start_time.elapsed();
    info!("Indexing complete. Total facts ingested: {total_facts} (Took {elapsed:.2?})");
    Ok(())
}
