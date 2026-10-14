// lena-cli/src/doc.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::{db::LenaDb, docgen::generate_static_docs};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, out_dir: &PathBuf, theme: &str) -> Result<()> {
    println!(
        "Generating static documentation ({theme} theme) to {}...",
        out_dir.display()
    );
    match generate_static_docs(db, out_dir, theme).await {
        Ok(()) => println!(
            "Documentation successfully generated! Open {}/index.html in your browser.",
            out_dir.display()
        ),
        Err(e) => eprintln!("Failed to generate documentation: {e}"),
    }
    Ok(())
}
