//! Unified Adapter Management

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::info;

pub async fn run(action: &str, name: Option<&str>) -> Result<()> {
    match action {
        "list" => list_adapters().await,
        "install" => {
            if let Some(n) = name {
                install_adapter(n).await
            } else {
                anyhow::bail!("Adapter name required. Example: lena adapter install rust");
            }
        }
        _ => anyhow::bail!("Invalid action. Use 'list' or 'install'."),
    }
}

async fn list_adapters() -> Result<()> {
    let bin_dir = get_bin_dir()?;
    println!("Checking for installed adapters...\n");

    let known_adapters = [
        "lena-adapter-rust",
        "lena-adapter-go",
        "lena-adapter-clojure",
        "lena-adapter-treesitter",
    ];

    let is_lena_repo = Path::new("Cargo.toml").exists();
    let mut found_any = false;

    for adapter in known_adapters {
        let mut local_bin_path = None;
        let mut lena_bin_path = None;
        let mut path_bin_path = None;

        // Check target/debug/ (if inside repo)
        if is_lena_repo {
            let p = PathBuf::from("target").join("debug").join(format!(
                "{}{}",
                adapter,
                std::env::consts::EXE_SUFFIX
            ));
            if p.exists() {
                local_bin_path = Some(p);
            }
        }

        // Check ~/.lena/bin/
        let managed_path = bin_dir.join(format!("{}{}", adapter, std::env::consts::EXE_SUFFIX));
        if managed_path.exists() {
            lena_bin_path = Some(managed_path);
        }

        // Check $PATH
        if let Ok(p) = which::which(adapter) {
            // Ensure we aren't just resolving the exact same file we found in ~/.lena/bin
            if lena_bin_path.as_ref() != Some(&p) {
                path_bin_path = Some(p);
            }
        }

        if local_bin_path.is_none() && lena_bin_path.is_none() && path_bin_path.is_none() {
            continue;
        }

        found_any = true;

        if let Some(p) = &local_bin_path {
            let version = get_adapter_version(p).await;
            println!("- {adapter} ({version}) [Local Dev: {}]", p.display());
        }

        if let Some(p) = &lena_bin_path {
            let version = get_adapter_version(p).await;
            println!("- {adapter} ({version}) [Managed: {}]", p.display());

            if local_bin_path.is_some() {
                tracing::warn!(
                    "  ⚠️  This managed installation is being shadowed by the local target/debug/ build."
                );
            }
        }

        if let Some(p) = &path_bin_path {
            let version = get_adapter_version(p).await;
            println!("- {adapter} ({version}) [System: {}]", p.display());

            if local_bin_path.is_some() || lena_bin_path.is_some() {
                tracing::warn!(
                    "  ⚠️  This system installation is being shadowed by a higher-priority installation."
                );
            }
        }
    }

    if !found_any {
        println!("No Leña adapters found in ~/.lena/bin/ or $PATH.");
    }

    Ok(())
}

async fn get_adapter_version(path: &PathBuf) -> String {
    match Command::new(path).arg("--version").output().await {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "unknown version".to_string(),
    }
}

async fn install_adapter(name: &str) -> Result<()> {
    let bin_dir = get_bin_dir()?;
    fs::create_dir_all(&bin_dir)?;

    info!("Installing lena-adapter-{name}...");

    match name {
        "rust" | "clj" | "treesitter" => {
            let crate_name = format!("lena-adapter-{name}");
            let lena_root = dirs::home_dir().context("No home directory")?.join(".lena");

            let status = Command::new("cargo")
                .arg("install")
                .arg(&crate_name)
                .arg("--root")
                .arg(&lena_root)
                .status()
                .await?;

            if status.success() {
                info!("Successfully installed {crate_name} to ~/.lena/bin/");
            } else {
                anyhow::bail!("Failed to install {crate_name} via Cargo.");
            }
        }
        "go" => {
            let local_main = PathBuf::from("./lena-adapter-go/src/main.go");

            let status = if local_main.exists() {
                info!("Local workspace detected. Building Go adapter from source...");
                Command::new("go")
                    .arg("build")
                    .arg("-o")
                    .arg(bin_dir.join("lena-adapter-go"))
                    .arg("./src")
                    .current_dir("./lena-adapter-go")
                    .status()
                    .await?
            } else {
                info!("Remote user mode. Installing Go adapter via standard toolchain...");
                Command::new("go")
                    .arg("install")
                    .arg("github.com/yeajustmars/lena/lena-adapter-go/src@main")
                    .env("GOBIN", &bin_dir)
                    .status()
                    .await?
            };

            if status.success() {
                info!("Successfully installed lena-adapter-go to ~/.lena/bin/");
            } else {
                anyhow::bail!("Failed to install lena-adapter-go.");
            }
        }
        _ => {
            anyhow::bail!("Unknown adapter '{name}'. Available: rust, clj, treesitter, go");
        }
    }
    Ok(())
}

pub fn get_bin_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".lena").join("bin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_bin_dir() {
        let bin_dir = get_bin_dir().expect("Failed to get bin dir");
        assert!(bin_dir.ends_with(".lena/bin"));
    }

    #[test]
    fn test_managed_adapter_names() {
        let supported = ["rust", "clj", "treesitter", "go"];
        assert_eq!(supported.len(), 4);
        assert!(supported.contains(&"go"));
        assert!(supported.contains(&"rust"));
    }
}
