// lena-core/src/docgen/mod.rs

use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub mod html;
pub mod ir;
pub mod mdbook;

use crate::db::LenaDb;
use crate::docgen::ir::{
    DocGraph, DocLanguage, DocModule, DocNeighborhood, DocPackage, DocSymbol, NeighborRef,
    SearchEntry, SymbolKind,
};

pub struct DocGenOptions {
    pub project_name: String,
    pub version: String,
    pub custom_css: Option<String>,
    pub exclude_patterns: Vec<String>,
}

pub struct DocGraphBuilder<'a> {
    db: &'a LenaDb,
    options: DocGenOptions,
}

impl<'a> DocGraphBuilder<'a> {
    pub fn new(db: &'a LenaDb, options: DocGenOptions) -> Self {
        Self { db, options }
    }

    pub async fn build(self) -> Result<DocGraph> {
        let raw_symbols = self.fetch_all_symbols().await?;

        let mut symbols_map = HashMap::new();
        for sym in &raw_symbols {
            symbols_map.insert(sym.fqsn.clone(), sym.clone());
        }

        let (callers_map, callees_map) = self.fetch_call_graph_edges(&symbols_map).await?;

        let mut languages: BTreeMap<String, DocLanguage> = BTreeMap::new();

        let mut search_index: Vec<SearchEntry> = Vec::new();

        let excludes: Vec<regex::Regex> = self
            .options
            .exclude_patterns
            .iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        for raw in raw_symbols {
            if excludes.iter().any(|re| re.is_match(&raw.file_path)) {
                continue;
            }

            let fqsn = raw.fqsn.clone();
            let callers = callers_map.get(&fqsn).cloned().unwrap_or_default();
            let callees = callees_map.get(&fqsn).cloned().unwrap_or_default();

            // Calculate context-aware display name (e.g. `Server.Close` or `Router::nest`)
            let display_name = if let Some(c) = &raw.container {
                match raw.language.to_lowercase().as_str() {
                    "go" | "java" | "javascript" | "python" | "c#" | "typescript" => {
                        format!("{}.{}", c, raw.name)
                    }
                    _ => format!("{}::{}", c, raw.name), // Rust, C++, PHP, etc.
                }
            } else {
                raw.name.clone()
            };

            let mut neighborhood = DocNeighborhood {
                callers,
                callees,
                mermaid_chart: None,
            };
            neighborhood.generate_mermaid(&display_name);

            // Skip external stubs from getting dedicated doc pages, search entries,
            // and being rendered in caller/callee graphs.
            if raw.is_external {
                continue;
            }

            let summary = raw
                .docstring
                .as_ref()
                .map(|d| d.lines().next().unwrap_or("").trim().to_string())
                .unwrap_or_default();

            let doc_symbol = DocSymbol {
                fqsn: raw.fqsn.clone(),
                name: display_name.clone(),
                kind: raw.kind,
                signature: raw.signature,
                docstring: raw.docstring,
                file_path: raw.file_path.clone(),
                line: raw.line,
                container: raw.container.clone(),
                submodule: raw.submodule.clone(),
                neighborhood,
            };

            let lang_entry = languages
                .entry(raw.language.clone())
                .or_insert_with(|| DocLanguage {
                    name: raw.language.clone(),
                    packages: BTreeMap::new(),
                });

            let pkg_entry = lang_entry
                .packages
                .entry(raw.package.clone())
                .or_insert_with(|| DocPackage {
                    name: raw.package.clone(),
                    language: raw.language.clone(),
                    modules: BTreeMap::new(),
                });

            let mod_entry = pkg_entry
                .modules
                .entry(raw.module.clone())
                .or_insert_with(|| DocModule {
                    name: raw.module.clone(),
                    namespace: raw.module.clone(),
                    path: raw.file_path.clone(),
                    docstring: None,
                    symbols: Vec::new(),
                });

            let href = format!(
                "{}/{}/{}.html#{}",
                raw.language, raw.package, raw.module, raw.name
            );

            search_index.push(SearchEntry {
                fqsn: raw.fqsn,
                name: display_name,
                kind: format!("{:?}", raw.kind),
                module: raw.module,
                summary,
                href,
            });

            mod_entry.symbols.push(doc_symbol);
        }

        // Sort symbols for deterministic layout
        for lang in languages.values_mut() {
            for pkg in lang.packages.values_mut() {
                for module in pkg.modules.values_mut() {
                    module.symbols.sort_by(|a, b| {
                        a.kind
                            .cmp(&b.kind)
                            .then_with(|| a.line.cmp(&b.line))
                            .then_with(|| a.name.cmp(&b.name))
                    });
                }
            }
        }

        Ok(DocGraph {
            project_name: self.options.project_name,
            version: self.options.version,
            languages,
            search_index,
        })
    }

    async fn fetch_all_symbols(&self) -> Result<Vec<RawSymbol>> {
        let query = "[:find (pull ?e [*]) :where [?e :symbol/fqsn ?fqsn]]";

        let results_json = self.db.query_json(query).await?;
        let rows: Vec<Value> = serde_json::from_str(&results_json)?;

        let mut symbols = Vec::new();
        for row in rows {
            let data = row.get("e").unwrap_or(&row);

            let fqsn = extract_string(data, "symbol/fqsn");
            if fqsn.is_empty() {
                continue;
            }

            let name = extract_string(data, "symbol/name");
            if name.is_empty() {
                continue;
            }

            let kind_str = extract_string(data, "symbol/kind")
                .replace(":type/", "")
                .replace(":kind/", "")
                .replace(':', "");

            let language = extract_string(data, "entity/language").replace(":lang/", "");

            let is_external = extract_bool(data, "symbol/external");

            let sig_raw = extract_string(data, "code/signature");
            let sig_str = if sig_raw.is_empty() {
                None
            } else {
                Some(sig_raw.replace("\\n", "\n"))
            };

            let doc_raw = extract_string(data, "doc/text");
            let doc_str = if doc_raw.is_empty() {
                None
            } else {
                Some(doc_raw.replace("\\n", "\n"))
            };

            let file_path = extract_string(data, "location/file");
            let line_val = extract_i64(data, "location/start-line");
            let line = if line_val > 0 {
                Some(line_val as usize)
            } else {
                None
            };

            let module = extract_string(data, "symbol/module");

            let submod_raw = extract_string(data, "symbol/submodule");
            let submodule = if submod_raw.is_empty() {
                None
            } else {
                Some(submod_raw)
            };

            let cont_raw = extract_string(data, "symbol/container");
            let container = if cont_raw.is_empty() {
                None
            } else {
                Some(cont_raw)
            };

            // Determine the top-level package boundary for UI grouping
            let mut package = module.split("::").next().unwrap_or(&module).to_string();
            if package == module {
                package = module.split('/').next().unwrap_or(&module).to_string();
            }
            if package == module {
                package = module.split('.').next().unwrap_or(&module).to_string();
            }
            if package.is_empty() {
                package = "root".to_string();
            }

            symbols.push(RawSymbol {
                fqsn,
                name,
                kind: SymbolKind::from_str(&kind_str),
                signature: sig_str,
                docstring: doc_str,
                file_path,
                line,
                language: if language.is_empty() {
                    "generic".to_string()
                } else {
                    language
                },
                package,
                module: if module.is_empty() {
                    "root".to_string()
                } else {
                    module
                },
                submodule,
                container,
                is_external,
            });
        }
        Ok(symbols)
    }

    async fn fetch_call_graph_edges(
        &self,
        symbols_map: &HashMap<String, RawSymbol>,
    ) -> Result<(
        HashMap<String, Vec<NeighborRef>>,
        HashMap<String, Vec<NeighborRef>>,
    )> {
        let query = r#"
            [:find ?caller_fqsn ?callee_fqsn
             :where
             [?c :call/caller ?caller_eid]
             [?c :call/callee ?callee_eid]
             [?caller_eid :symbol/fqsn ?caller_fqsn]
             [?callee_eid :symbol/fqsn ?callee_fqsn]]
        "#;

        let results_json = self.db.query_json(query).await?;
        let rows: Vec<Value> = serde_json::from_str(&results_json)?;

        let mut callers_map: HashMap<String, Vec<NeighborRef>> = HashMap::new();
        let mut callees_map: HashMap<String, Vec<NeighborRef>> = HashMap::new();

        for row in rows {
            let (caller_fqsn, callee_fqsn) = match &row {
                Value::Array(arr) => {
                    let caller = arr
                        .first()
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let callee = arr
                        .get(1)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    (caller, callee)
                }
                Value::Object(_) => {
                    let caller = {
                        let s = extract_string(&row, "?caller_fqsn");
                        if s.is_empty() {
                            extract_string(&row, "caller_fqsn")
                        } else {
                            s
                        }
                    };
                    let callee = {
                        let s = extract_string(&row, "?callee_fqsn");
                        if s.is_empty() {
                            extract_string(&row, "callee_fqsn")
                        } else {
                            s
                        }
                    };
                    (caller, callee)
                }
                _ => continue,
            };

            if caller_fqsn.is_empty() || callee_fqsn.is_empty() {
                continue;
            }

            let caller_sym = symbols_map.get(&caller_fqsn);
            let callee_sym = symbols_map.get(&callee_fqsn);

            let caller_is_external = caller_sym.map_or(true, |s| s.is_external);
            let callee_is_external = callee_sym.map_or(true, |s| s.is_external);

            // Fetch display names to show containers in Mermaid graphs
            let caller_name = caller_sym
                .map(|s| {
                    if let Some(c) = &s.container {
                        format!("{}::{}", c, s.name)
                    } else {
                        s.name.clone()
                    }
                })
                .unwrap_or_else(|| caller_fqsn.clone());
            let caller_file = caller_sym.map(|s| s.file_path.clone()).unwrap_or_default();

            let callee_name = callee_sym
                .map(|s| {
                    if let Some(c) = &s.container {
                        format!("{}::{}", c, s.name)
                    } else {
                        s.name.clone()
                    }
                })
                .unwrap_or_else(|| callee_fqsn.clone());
            let callee_file = callee_sym.map(|s| s.file_path.clone()).unwrap_or_default();

            let caller_ref = NeighborRef {
                fqsn: caller_fqsn.clone(),
                name: caller_name,
                file_path: caller_file,
            };

            let callee_ref = NeighborRef {
                fqsn: callee_fqsn.clone(),
                name: callee_name,
                file_path: callee_file,
            };

            if !caller_is_external {
                callers_map
                    .entry(callee_fqsn.clone())
                    .or_default()
                    .push(caller_ref);
            }

            if !callee_is_external {
                callees_map.entry(caller_fqsn).or_default().push(callee_ref);
            }
        }

        Ok((callers_map, callees_map))
    }
}

#[derive(Clone)]
struct RawSymbol {
    fqsn: String,
    name: String,
    kind: SymbolKind,
    signature: Option<String>,
    docstring: Option<String>,
    file_path: String,
    line: Option<usize>,
    language: String,
    package: String,
    module: String,
    submodule: Option<String>,
    container: Option<String>,
    is_external: bool,
}

fn extract_string(val: &Value, key: &str) -> String {
    let key_with_colon = format!(":{key}");
    val.get(&key_with_colon)
        .or_else(|| val.get(key))
        .and_then(|v| {
            v.as_str().map(ToString::to_string).or_else(|| {
                v.as_array()
                    .and_then(|arr| arr.first()?.as_str().map(ToString::to_string))
            })
        })
        .unwrap_or_default()
}

fn extract_i64(val: &Value, key: &str) -> i64 {
    let key_with_colon = format!(":{key}");
    val.get(&key_with_colon)
        .or_else(|| val.get(key))
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_array().and_then(|arr| arr.first()?.as_i64()))
        })
        .unwrap_or(0)
}

fn extract_bool(val: &Value, key: &str) -> bool {
    let key_with_colon = format!(":{key}");
    val.get(&key_with_colon)
        .or_else(|| val.get(key))
        .and_then(|v| {
            v.as_bool()
                .or_else(|| v.as_array().and_then(|arr| arr.first()?.as_bool()))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::LenaDb;
    use mesodb_core::transactor::Fact;
    use mesodb_core::types::Value as MesoValue;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_docgraph_builder_extraction() {
        let dir = tempdir().unwrap();
        let db = LenaDb::open(dir.path().join("docgen.db")).unwrap();

        // Seed mock symbol with explicitly defined modularity attributes
        db.inner()
            .transact(vec![
                Fact {
                    e: 1,
                    ident: ":symbol/fqsn".into(),
                    v: MesoValue::String("rust:crate::func".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/name".into(),
                    v: MesoValue::String("func".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":entity/language".into(),
                    v: MesoValue::String("Rust".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/module".into(),
                    v: MesoValue::String("core".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/container".into(),
                    v: MesoValue::String("MyStruct".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
            ])
            .await
            .unwrap();

        let options = DocGenOptions {
            project_name: "TestProject".into(),
            version: "1.0.0".into(),
            custom_css: None,
        };

        let builder = DocGraphBuilder::new(&db, options);
        let graph = builder.build().await.unwrap();

        assert_eq!(graph.project_name, "TestProject");
        assert!(graph.languages.contains_key("Rust"));

        let rust_lang = graph.languages.get("Rust").unwrap();
        assert!(rust_lang.packages.contains_key("core"));

        let core_pkg = rust_lang.packages.get("core").unwrap();
        let module = core_pkg.modules.get("core").unwrap();
        let symbol = &module.symbols[0];

        // Assert container display name logic triggered
        assert_eq!(symbol.name, "MyStruct::func");
        assert_eq!(symbol.container, Some("MyStruct".to_string()));
    }

    #[tokio::test]
    async fn test_docgraph_skips_external_symbols() {
        let dir = tempdir().unwrap();
        let db = LenaDb::open(dir.path().join("docgen_ext.db")).unwrap();

        db.inner()
            .transact(vec![
                Fact {
                    e: 1,
                    ident: ":symbol/fqsn".into(),
                    v: MesoValue::String("go:fmt/Println".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/name".into(),
                    v: MesoValue::String("Println".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/external".into(),
                    v: MesoValue::Boolean(true),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
            ])
            .await
            .unwrap();

        let options = DocGenOptions {
            project_name: "Test".into(),
            version: "1.0.0".into(),
            custom_css: None,
        };

        let graph = DocGraphBuilder::new(&db, options).build().await.unwrap();

        assert!(
            graph.languages.is_empty(),
            "External symbols should not pollute the main documentation tree"
        );
        assert!(
            graph.search_index.is_empty(),
            "External symbols should not appear in search results"
        );
    }

    #[tokio::test]
    async fn test_docgraph_language_specific_display_name_formatting() {
        let dir = tempdir().unwrap();
        let db = LenaDb::open(dir.path().join("docgen_lang.db")).unwrap();

        // Seed a Go method (should format as Server.Close)
        db.inner()
            .transact(vec![
                Fact {
                    e: 10,
                    ident: ":symbol/fqsn".into(),
                    v: MesoValue::String("go:net/http.Server.Close".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 10,
                    ident: ":symbol/name".into(),
                    v: MesoValue::String("Close".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 10,
                    ident: ":entity/language".into(),
                    v: MesoValue::String("go".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 10,
                    ident: ":symbol/module".into(),
                    v: MesoValue::String("net/http".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 10,
                    ident: ":symbol/container".into(),
                    v: MesoValue::String("Server".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 10,
                    ident: ":location/file".into(),
                    v: MesoValue::String("server.go".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
            ])
            .await
            .unwrap();

        let options = DocGenOptions {
            project_name: "GoTest".into(),
            version: "1.0.0".into(),
            custom_css: None,
        };

        let graph = DocGraphBuilder::new(&db, options).build().await.unwrap();
        let go_lang = graph.languages.get("go").unwrap();
        let pkg = go_lang.packages.get("net").unwrap();
        let module = pkg.modules.get("net/http").unwrap();

        assert_eq!(module.symbols[0].name, "Server.Close");
    }

    #[tokio::test]
    async fn test_docgraph_call_edges_exclude_external_stubs() {
        let dir = tempdir().unwrap();
        let db = LenaDb::open(dir.path().join("docgen_edges.db")).unwrap();

        // Internal caller referencing external callee
        db.inner()
            .transact(vec![
                Fact {
                    e: 1,
                    ident: ":symbol/fqsn".into(),
                    v: MesoValue::String("rust:app::main".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/name".into(),
                    v: MesoValue::String("main".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/module".into(),
                    v: MesoValue::String("app".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":entity/language".into(),
                    v: MesoValue::String("rust".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":location/file".into(),
                    v: MesoValue::String("src/main.rs".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                // External stub
                Fact {
                    e: 2,
                    ident: ":symbol/fqsn".into(),
                    v: MesoValue::String("rust:std::process::exit".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 2,
                    ident: ":symbol/name".into(),
                    v: MesoValue::String("exit".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 2,
                    ident: ":symbol/external".into(),
                    v: MesoValue::Boolean(true),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                // Call edge
                Fact {
                    e: 3,
                    ident: ":entity/type".into(),
                    v: MesoValue::String(":type/call-site".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 3,
                    ident: ":call/caller".into(),
                    v: MesoValue::Ref(1),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 3,
                    ident: ":call/callee".into(),
                    v: MesoValue::Ref(2),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
            ])
            .await
            .unwrap();

        let options = DocGenOptions {
            project_name: "EdgeTest".into(),
            version: "1.0.0".into(),
            custom_css: None,
        };

        let graph = DocGraphBuilder::new(&db, options).build().await.unwrap();
        let app_module = graph
            .languages
            .get("rust")
            .unwrap()
            .packages
            .get("app")
            .unwrap()
            .modules
            .get("app")
            .unwrap();
        let main_sym = &app_module.symbols[0];

        assert_eq!(
            main_sym.neighborhood.callees.len(),
            0,
            "External stubs should be excluded from the generated documentation graph"
        );
    }
}
