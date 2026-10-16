// lena-cli/src/doc.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use lena_core::db::LenaDb;
use lena_core::docgen::{
    DocGenOptions, DocGraphBuilder, html::HtmlRenderer, mdbook::MdBookEmitter,
};
use std::path::PathBuf;
use std::sync::Arc;

pub async fn run(db: &Arc<LenaDb>, out_dir: &PathBuf, theme: &str) -> Result<()> {
    println!(
        "Generating static documentation ({theme} theme) to {}...",
        out_dir.display()
    );

    let options = DocGenOptions {
        project_name: "Lena Project".to_string(), // Can be exposed via CLI args later
        version: env!("CARGO_PKG_VERSION").to_string(),
        custom_css: None,
    };

    let builder = DocGraphBuilder::new(db, options);
    let graph = builder.build().await?;

    if theme == "mdbook" || theme == "book" {
        let emitter = MdBookEmitter::new(&graph);
        emitter.emit(out_dir)?;
        println!(
            "Documentation successfully generated! Check the {}/ directory for the mdBook source.",
            out_dir.display()
        );
    } else {
        let renderer = HtmlRenderer::new(&graph)?;
        renderer.render_lena_book(out_dir)?;
        println!(
            "Documentation successfully generated! Open {}/index.html in your browser.",
            out_dir.display()
        );
    }

    Ok(())
}
