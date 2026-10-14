// lena-cli/src/skeleton.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::{
    db::LenaDb,
    skeleton::{SkeletonFormat, generate_skeleton},
};
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, fqsn: &str, depth: usize, format: &str) -> Result<()> {
    let skel_format = match format.to_lowercase().as_str() {
        "mermaid" => SkeletonFormat::Mermaid,
        "dot" => SkeletonFormat::Dot,
        "indented" | "text" => SkeletonFormat::Indented,
        _ => {
            eprintln!("Invalid format. Use 'indented', 'mermaid', or 'dot'.");
            return Ok(());
        }
    };

    match generate_skeleton(db, fqsn, depth, skel_format).await {
        Ok(output) => println!("{output}"),
        Err(e) => eprintln!("Failed to generate skeleton: {e}"),
    }
    Ok(())
}
