use crate::docgen::ir::DocGraph;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct MdBookEmitter<'a> {
    graph: &'a DocGraph,
}

impl<'a> MdBookEmitter<'a> {
    pub fn new(graph: &'a DocGraph) -> Self {
        Self { graph }
    }

    pub fn emit(&self, out_dir: &Path) -> Result<()> {
        let src_dir = out_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        let book_toml = format!(
            r#"[book]
authors = ["Lena Graph Engine"]
language = "en"
multilingual = false
src = "src"
title = "{}"

[output.html]
git-repository-url = "https://github.com"
"#,
            self.graph.project_name
        );
        fs::write(out_dir.join("book.toml"), book_toml)?;

        let mut summary = String::from("# Summary\n\n");
        summary.push_str("[Overview](README.md)\n\n");

        fs::write(
            src_dir.join("README.md"),
            format!("# {}\n\nSystem Documentation", self.graph.project_name),
        )?;

        for (lang_name, lang) in &self.graph.languages {
            summary.push_str(&format!("- [{lang_name}]()\n"));
            for (pkg_name, pkg) in &lang.packages {
                summary.push_str(&format!("  - [{pkg_name}]()\n"));
                for (mod_name, module) in &pkg.modules {
                    let rel_path = format!("{lang_name}_{pkg_name}_{mod_name}.md");
                    summary.push_str(&format!("    - [{mod_name}]({rel_path})\n"));

                    let mut chapter = format!("# Module `{mod_name}`\n\n");
                    chapter.push_str(&format!("**Source**: `{}`\n\n", module.path));

                    for sym in &module.symbols {
                        chapter.push_str(&format!("## `{}`\n\n", sym.name));
                        if let Some(ref sig) = sym.signature {
                            chapter.push_str(&format!("```rust\n{sig}\n```\n\n"));
                        }
                        if let Some(ref doc) = sym.docstring {
                            chapter.push_str(&format!("{doc}\n\n"));
                        }
                        if let Some(ref mermaid) = sym.neighborhood.mermaid_chart {
                            chapter.push_str(&format!("```mermaid\n{mermaid}\n```\n\n"));
                        }
                    }
                    fs::write(src_dir.join(&rel_path), chapter)?;
                }
            }
        }

        fs::write(src_dir.join("SUMMARY.md"), summary)?;

        if which::which("mdbook").is_ok() {
            Command::new("mdbook")
                .arg("build")
                .arg(out_dir)
                .status()
                .context("Failed to run mdbook build")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docgen::ir::{
        DocLanguage, DocModule, DocNeighborhood, DocPackage, DocSymbol, SymbolKind,
    };
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn test_mdbook_emission() {
        let symbols = vec![DocSymbol {
            fqsn: "rust:core::run".into(),
            name: "run".into(),
            kind: SymbolKind::Function,
            signature: Some("fn run()".into()),
            docstring: Some("Runs the thing".into()),
            file_path: "src/lib.rs".into(),
            line: Some(10),
            neighborhood: DocNeighborhood::default(),
        }];

        let mut modules = BTreeMap::new();
        modules.insert(
            "core".into(),
            DocModule {
                name: "core".into(),
                path: "src/core.rs".into(),
                docstring: None,
                symbols,
            },
        );

        let mut packages = BTreeMap::new();
        packages.insert(
            "app".into(),
            DocPackage {
                name: "app".into(),
                language: "Rust".into(),
                modules,
            },
        );

        let mut languages = BTreeMap::new();
        languages.insert(
            "Rust".into(),
            DocLanguage {
                name: "Rust".into(),
                packages,
            },
        );

        let graph = DocGraph {
            project_name: "Lena".into(),
            version: "1.0".into(),
            languages,
            search_index: vec![],
        };

        let emitter = MdBookEmitter::new(&graph);
        let dir = tempdir().unwrap();

        // Disable execution of the actual mdbook binary during tests
        let _ = emitter.emit(dir.path());

        assert!(dir.path().join("book.toml").exists());
        assert!(dir.path().join("src/SUMMARY.md").exists());
        assert!(dir.path().join("src/Rust_app_core.md").exists());

        let summary = fs::read_to_string(dir.path().join("src/SUMMARY.md")).unwrap();
        assert!(summary.contains("[core](Rust_app_core.md)"));

        let chapter = fs::read_to_string(dir.path().join("src/Rust_app_core.md")).unwrap();
        assert!(chapter.contains("## `run`"));
        assert!(chapter.contains("fn run()"));
    }
}
