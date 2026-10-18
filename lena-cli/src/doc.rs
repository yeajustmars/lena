// lena-cli/src/doc.rs

// #![deny(warnings)]
// #![warn(clippy::all, clippy::pedantic)]
// TODO: put clippy::pendantic in all modules and harden

use anyhow::Result;
use lena_core::db::LenaDb;
use lena_core::docgen::{
    DocGenOptions, DocGraphBuilder, html::HtmlRenderer, mdbook::MdBookEmitter,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub async fn run(
    db: &Arc<LenaDb>,
    out_dir: &Path,
    format: &str,
    css_path: Option<&PathBuf>,
    github_actions: bool,
) -> Result<()> {
    println!(
        "Generating static documentation ({format} format) to {}...",
        out_dir.display()
    );

    let custom_css = if let Some(path) = css_path {
        Some(fs::read_to_string(path)?)
    } else {
        None
    };

    let options = DocGenOptions {
        project_name: "Lena Project".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        custom_css: custom_css.clone(),
    };

    let builder = DocGraphBuilder::new(db, options);
    let graph = builder.build().await?;

    if format == "mdbook" {
        let emitter = MdBookEmitter::new(&graph);
        emitter.emit(out_dir)?;
    } else if format == "single-page" {
        let renderer = HtmlRenderer::new(&graph, custom_css)?;
        renderer.render_single_page(out_dir)?;
    } else {
        // Defaults to "book" (the mobile-first responsive lena-book)
        let renderer = HtmlRenderer::new(&graph, custom_css)?;
        renderer.render_lena_book(out_dir)?;
    }

    if github_actions {
        let workflow_dir = PathBuf::from(".github/workflows");
        fs::create_dir_all(&workflow_dir)?;
        let workflow_path = workflow_dir.join("docs.yml");

        let workflow_content = r#"name: Deploy Documentation to GitHub Pages

on:
  push:
    branches: ["main", "master"]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: "pages"
  cancel-in-progress: false

jobs:
  deploy:
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      - name: Setup Pages
        uses: actions/configure-pages@v4
      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: './dist'
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
"#;
        fs::write(&workflow_path, workflow_content)?;
        println!(
            "Generated GitHub Pages workflow at {}",
            workflow_path.display()
        );
    }

    println!("Documentation successfully generated!");
    Ok(())
}
