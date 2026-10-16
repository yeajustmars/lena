use crate::docgen::ir::DocGraph;
use anyhow::Result;
use minijinja::{Environment, context};
use std::fs;
use std::path::Path;

const BASE_PAGE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{{ page_title }} - {{ project_name }}</title>
  <link rel="stylesheet" href="{{ root_rel }}assets/modern.css">
  <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
  <script>mermaid.initialize({ startOnLoad: true, theme: 'neutral' });</script>
</head>
<body class="theme-light">
  <div class="layout">
    <nav class="sidebar">
      <div class="sidebar-header">
        <a href="{{ root_rel }}index.html">{{ project_name }}</a>
        <span class="version">{{ version }}</span>
      </div>
      <input type="text" id="search-box" placeholder="Search symbols (/) ..." autocomplete="off" />
      <div id="search-results" class="search-popover hidden"></div>
      <ul class="nav-tree">
        {% for lang_name, lang in languages | items %}
        <li class="nav-language">
          <span class="group-title">{{ lang_name }}</span>
          {% for pkg_name, pkg in lang.packages | items %}
          <div class="nav-package">
            <span class="pkg-title">{{ pkg_name }}</span>
            <ul class="nav-modules">
              {% for mod_name, mod in pkg.modules | items %}
              <li><a href="{{ root_rel }}{{ lang_name }}/{{ pkg_name }}/{{ mod_name }}.html">{{ mod_name }}</a></li>
              {% endfor %}
            </ul>
          </div>
          {% endfor %}
        </li>
        {% endfor %}
      </ul>
    </nav>
    <main class="content">
      {% block content %}{% endblock %}
    </main>
  </div>
  <script src="{{ root_rel }}assets/search.js"></script>
</body>
</html>"#;

pub struct HtmlRenderer<'a> {
    graph: &'a DocGraph,
    env: Environment<'static>,
}

impl<'a> HtmlRenderer<'a> {
    pub fn new(graph: &'a DocGraph) -> Result<Self> {
        let mut env = Environment::new();
        env.add_template("base.html", BASE_PAGE_TEMPLATE)?;
        Ok(Self { graph, env })
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
        // MiniJinja renders module template extending base.html
        let template_str = r#"
        {% extends "base.html" %}
        {% block content %}
          <h1>{{ module.name }}</h1>
          <p class="file-path">Source: <code>{{ module.path }}</code></p>
          {% if module.docstring %}
            <div class="module-doc">{{ module.docstring }}</div>
          {% endif %}
          <hr />
          <section class="symbols">
            {% for sym in module.symbols %}
              <div class="symbol-card" id="{{ sym.name }}">
                <div class="symbol-header">
                  <span class="badge {{ sym.kind.badge_class() }}">{{ sym.kind }}</span>
                  <h3 class="symbol-name">{{ sym.name }}</h3>
                  {% if sym.line %}<span class="symbol-line">Line {{ sym.line }}</span>{% endif %}
                </div>
                {% if sym.signature %}
                  <pre class="symbol-sig"><code>{{ sym.signature }}</code></pre>
                {% endif %}
                {% if sym.docstring %}
                  <div class="symbol-desc">{{ sym.docstring }}</div>
                {% endif %}
                {% if sym.neighborhood.mermaid_chart %}
                  <div class="neighborhood-graph">
                    <h4>Call Neighborhood</h4>
                    <pre class="mermaid">{{ sym.neighborhood.mermaid_chart }}</pre>
                  </div>
                {% endif %}
              </div>
            {% endfor %}
          </section>
        {% endblock %}
        "#;

        let mut env = self.env.clone();
        env.add_template("module.html", template_str)?;
        let tmpl = env.get_template("module.html")?;

        let rendered = tmpl.render(context! {
            project_name => self.graph.project_name,
            version => self.graph.version,
            languages => self.graph.languages,
            module => module,
            page_title => module.name,
            root_rel => "../../",
        })?;

        Ok(rendered)
    }

    fn render_landing_page(&self, out_dir: &Path) -> Result<()> {
        let landing_tmpl = r#"
        {% extends "base.html" %}
        {% block content %}
          <div class="hero">
            <h1>{{ project_name }} Documentation</h1>
            <p class="subtitle">Version: {{ version }}</p>
          </div>
          <div class="overview-grid">
            {% for lang_name, lang in languages | items %}
              <div class="card">
                <h2>{{ lang_name }}</h2>
                <ul>
                  {% for pkg_name, pkg in lang.packages | items %}
                    <li><strong>{{ pkg_name }}</strong> ({{ pkg.modules | length }} modules)</li>
                  {% endfor %}
                </ul>
              </div>
            {% endfor %}
          </div>
        {% endblock %}
        "#;

        let mut env = self.env.clone();
        env.add_template("index.html", landing_tmpl)?;
        let tmpl = env.get_template("index.html")?;

        let rendered = tmpl.render(context! {
            project_name => self.graph.project_name,
            version => self.graph.version,
            languages => self.graph.languages,
            page_title => "Overview",
            root_rel => "./",
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

        let renderer = HtmlRenderer::new(&graph).unwrap();
        let dir = tempdir().unwrap();

        renderer.render_lena_book(dir.path()).unwrap();

        assert!(dir.path().join("index.html").exists());
        assert!(dir.path().join("assets/modern.css").exists());
        assert!(dir.path().join("assets/search_index.json").exists());
        assert!(dir.path().join("Rust/crate/main.html").exists());
    }
}
