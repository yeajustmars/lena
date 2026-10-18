// lena-cli/src/prompt.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::{
    config::PromptConfig,
    db::LenaDb,
    exporter::{ExportFormat, export_prompt},
};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn run(
    db: &Arc<LenaDb>,
    fqsn: &str,
    format: &str,
    template_path: Option<&PathBuf>,
    config: PromptConfig,
) -> Result<()> {
    let export_format = match format.to_lowercase().as_str() {
        "md" | "markdown" => ExportFormat::Markdown,
        "xml" => ExportFormat::Xml,
        "template" => {
            let Some(path) = template_path else {
                eprintln!("Error: --template path is required when format is 'template'");
                return Ok(());
            };
            let tpl_str = std::fs::read_to_string(path)?;
            ExportFormat::Template(tpl_str)
        }
        _ => {
            eprintln!("Invalid format. Use 'xml', 'md', or 'template'.");
            return Ok(());
        }
    };

    match export_prompt(db, fqsn, export_format, &config).await {
        Ok(output) => println!("{output}"),
        Err(e) => eprintln!("Failed to export prompt: {e}"),
    }
    Ok(())
}
