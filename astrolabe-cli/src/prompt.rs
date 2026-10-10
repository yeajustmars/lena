// astrolabe-cli/src/prompt.rs

use anyhow::Result;
use astrolabe_core::{
    db::AstrolabeDb,
    exporter::{ExportFormat, export_prompt},
};
use std::sync::Arc;

pub async fn run(db: &Arc<AstrolabeDb>, fqsn: &str, format: &str) -> Result<()> {
    let export_format = match format.to_lowercase().as_str() {
        "md" | "markdown" => ExportFormat::Markdown,
        "xml" => ExportFormat::Xml,
        _ => {
            eprintln!("Invalid format. Use 'xml' or 'md'.");
            return Ok(());
        }
    };

    match export_prompt(db, fqsn, export_format).await {
        Ok(output) => println!("{output}"),
        Err(e) => eprintln!("Failed to export prompt: {e}"),
    }
    Ok(())
}
