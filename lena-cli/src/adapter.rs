//! Unified Adapter Management

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
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
    println!(
        "Checking for installed adapters in {}...\n",
        bin_dir.display()
    );

    if !bin_dir.exists() {
        println!("No adapters installed in ~/.lena/bin/ yet.");
        return Ok(());
    }

    let mut entries = fs::read_dir(&bin_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect::<Vec<_>>();

    entries.sort();

    for path in entries {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if filename.starts_with("lena-adapter-") {
            // Attempt a version handshake
            let version = match Command::new(&path).arg("--version").output().await {
                Ok(out) if out.status.success() => {
                    String::from_utf8_lossy(&out.stdout).trim().to_string()
                }
                _ => "unknown version".to_string(),
            };
            println!("- {filename} ({version})");
        }
    }
    Ok(())
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
            // Force Go to compile and drop the binary directly into our managed directory
            let status = Command::new("go")
                .arg("install")
                .arg("github.com/lena-project/lena-adapter-go@latest")
                .env("GOBIN", &bin_dir)
                .status()
                .await?;

            if status.success() {
                info!("Successfully installed lena-adapter-go to ~/.lena/bin/");
            } else {
                anyhow::bail!("Failed to install lena-adapter-go via standard Go toolchain.");
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
