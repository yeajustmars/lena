// lena-core/src/docgen/html.rs

use crate::docgen::ir::DocGraph;
use anyhow::Result;
use minijinja::{Environment, context};
use std::fs;
use std::path::Path;

pub struct HtmlRenderer<'a> {
    graph: &'a DocGraph,
    env: Environment<'static>,
    custom_css: Option<String>,
}

impl<'a> HtmlRenderer<'a> {
    pub fn new(graph: &'a DocGraph, custom_css: Option<String>) -> Result<Self> {
        let mut env = Environment::new();

        // Register the badge mapping as a MiniJinja filter
        env.add_filter("badge_class", |kind: String| -> String {
            crate::docgen::ir::SymbolKind::from_str(&kind)
                .badge_class()
                .to_string()
        });

        env.add_template("base.html", include_str!("../../templates/base.html"))?;
        env.add_template(
            "single_page.html",
            include_str!("../../templates/single_page.html"),
        )?;
        env.add_template("module.html", include_str!("../../templates/module.html"))?;
        env.add_template("landing.html", include_str!("../../templates/landing.html"))?;
        Ok(Self {
            graph,
            env,
            custom_css,
        })
    }

    pub fn render_single_page(&self, out_dir: &Path) -> Result<()> {
        fs::create_dir_all(out_dir)?;

        let tmpl = self.env.get_template("single_page.html")?;
        let rendered = tmpl.render(context! {
            project_name => self.graph.project_name,
            version => self.graph.version,
            languages => self.graph.languages,
            builtin_css => include_str!("assets/modern.css"),
            custom_css => self.custom_css.clone(),
        })?;

        fs::write(out_dir.join("single-page.html"), rendered)?;
        Ok(())
    }

    pub fn render_lena_book(&self, out_dir: &Path) -> Result<()> {
        fs::create_dir_all(out_dir.join("assets"))?;

        // 1. Write search index bundle
        let search_json = serde_json::to_string(&self.graph.search_index)?;
        fs::write(out_dir.join("assets/search_index.json"), search_json)?;

        // 2. Write embedded static CSS and JS
        fs::write(
            out_dir.join("assets/modern.css"),
            include_str!("assets/modern.css"),
        )?;
        fs::write(
            out_dir.join("assets/search.js"),
            include_str!("assets/search.js"),
        )?;

        // 3. Render module pages
        for (lang_name, lang) in &self.graph.languages {
            for (pkg_name, pkg) in &lang.packages {
                let pkg_dir = out_dir.join(lang_name).join(pkg_name);
                fs::create_dir_all(&pkg_dir)?;

                for (mod_name, module) in &pkg.modules {
                    let page_file = pkg_dir.join(format!("{mod_name}.html"));
                    let content = self.render_module_page(module, lang_name, pkg_name)?;
                    fs::write(page_file, content)?;
                }
            }
        }

        // 4. Render Index Landing Page
        self.render_landing_page(out_dir)?;

        Ok(())
    }

    fn render_module_page(
        &self,
        module: &crate::docgen::ir::DocModule,
        _lang: &str,
        _pkg: &str,
    ) -> Result<String> {
        let tmpl = self.env.get_template("module.html")?;
        let rendered = tmpl.render(context! {
            project_name => self.graph.project_name,
            version => self.graph.version,
            languages => self.graph.languages,
            module => module,
            page_title => module.name,
            root_rel => "../../",
            custom_css => self.custom_css.clone(),
        })?;

        Ok(rendered)
    }

    fn render_landing_page(&self, out_dir: &Path) -> Result<()> {
        let tmpl = self.env.get_template("landing.html")?;
        let rendered = tmpl.render(context! {
            project_name => self.graph.project_name,
            version => self.graph.version,
            languages => self.graph.languages,
            page_title => "Overview",
            root_rel => "./",
            custom_css => self.custom_css.clone(),
        })?;

        fs::write(out_dir.join("index.html"), rendered)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docgen::ir::{DocLanguage, DocModule, DocPackage};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn test_html_renderer_emission() {
        let mut modules = BTreeMap::new();
        modules.insert(
            "main".into(),
            DocModule {
                name: "main".into(),
                path: "src/main.rs".into(),
                docstring: None,
                symbols: vec![],
            },
        );

        let mut packages = BTreeMap::new();
        packages.insert(
            "crate".into(),
            DocPackage {
                name: "crate".into(),
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
            project_name: "Test".into(),
            version: "0.1".into(),
            languages,
            search_index: vec![],
        };

        // Initialize renderer with custom CSS
        let renderer =
            HtmlRenderer::new(&graph, Some("body { background: black; }".to_string())).unwrap();
        let dir = tempdir().unwrap();

        // 1. Test Multi-page build
        renderer.render_lena_book(dir.path()).unwrap();

        assert!(dir.path().join("index.html").exists());
        assert!(dir.path().join("assets/modern.css").exists());
        assert!(dir.path().join("assets/search_index.json").exists());
        assert!(dir.path().join("Rust/crate/main.html").exists());

        // Verify custom CSS injection in multi-page
        let index_content = fs::read_to_string(dir.path().join("index.html")).unwrap();
        assert!(index_content.contains("body { background: black; }"));

        // 2. Test Single-page build
        renderer.render_single_page(dir.path()).unwrap();
        assert!(dir.path().join("single-page.html").exists());

        // Verify custom CSS injection in single-page
        let single_page_content = fs::read_to_string(dir.path().join("single-page.html")).unwrap();
        assert!(single_page_content.contains("body { background: black; }"));
    }
}
