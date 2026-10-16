pub mod html;
pub mod ir;
pub mod mdbook;

use crate::db::LenaDb;
use crate::docgen::ir::{
    DocGraph, DocLanguage, DocModule, DocNeighborhood, DocPackage, DocSymbol, NeighborRef,
    SearchEntry, SymbolKind,
};
use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub struct DocGenOptions {
    pub project_name: String,
    pub version: String,
    pub custom_css: Option<String>,
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

        for raw in raw_symbols {
            let fqsn = raw.fqsn.clone();
            let callers = callers_map.get(&fqsn).cloned().unwrap_or_default();
            let callees = callees_map.get(&fqsn).cloned().unwrap_or_default();

            let mut neighborhood = DocNeighborhood {
                callers,
                callees,
                mermaid_chart: None,
            };
            neighborhood.generate_mermaid(&raw.name);

            let summary = raw
                .docstring
                .as_ref()
                .map(|d| d.lines().next().unwrap_or("").trim().to_string())
                .unwrap_or_default();

            let doc_symbol = DocSymbol {
                fqsn: raw.fqsn.clone(),
                name: raw.name.clone(),
                kind: raw.kind,
                signature: raw.signature,
                docstring: raw.docstring,
                file_path: raw.file_path.clone(),
                line: raw.line,
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
                    path: raw.file_path.clone(),
                    docstring: None,
                    symbols: Vec::new(),
                });

            let href = format!(
                "{}/{}/{}.html#{}",
                raw.language,
                raw.package,
                raw.module.replace("::", "/"),
                raw.name
            );

            search_index.push(SearchEntry {
                fqsn: raw.fqsn,
                name: raw.name,
                kind: format!("{:?}", raw.kind),
                module: raw.module,
                summary,
                href,
            });

            mod_entry.symbols.push(doc_symbol);
        }

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
            let kind_str = extract_string(data, "symbol/kind").replace(":type/", "");
            let language = extract_string(data, "entity/language").replace(":lang/", "");

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

            // Normalize file path by stripping leading "./" or "/"
            let mut file_path = extract_string(data, "location/file");
            while file_path.starts_with("./") || file_path.starts_with('/') {
                if file_path.starts_with("./") {
                    file_path = file_path[2..].to_string();
                } else if file_path.starts_with('/') {
                    file_path = file_path[1..].to_string();
                }
            }

            let line_val = extract_i64(data, "location/start-line");
            let line = if line_val > 0 {
                Some(line_val as usize)
            } else {
                None
            };

            let mut package = extract_string(data, "symbol/namespace");
            if package.is_empty() {
                // Fallback: Use the top-level directory from the normalized file path
                package = file_path.split('/').next().unwrap_or("root").to_string();
                if package.is_empty() || package == "." {
                    package = "root".to_string();
                }
            }

            let module = if file_path.is_empty() {
                "main".to_string()
            } else {
                file_path.clone()
            };

            symbols.push(RawSymbol {
                fqsn,
                name: if name.is_empty() {
                    "Unnamed".to_string()
                } else {
                    name
                },
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
                module,
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
            let caller_fqsn = extract_string(&row, "?caller_fqsn");
            let callee_fqsn = extract_string(&row, "?callee_fqsn");

            if caller_fqsn.is_empty() || callee_fqsn.is_empty() {
                continue;
            }

            let caller_sym = symbols_map.get(&caller_fqsn);
            let callee_sym = symbols_map.get(&callee_fqsn);

            let caller_name = caller_sym
                .map(|s| s.name.clone())
                .unwrap_or_else(|| caller_fqsn.clone());
            let caller_file = caller_sym.map(|s| s.file_path.clone()).unwrap_or_default();

            let callee_name = callee_sym
                .map(|s| s.name.clone())
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

            callers_map
                .entry(callee_fqsn.clone())
                .or_default()
                .push(caller_ref);
            callees_map.entry(caller_fqsn).or_default().push(callee_ref);
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

        // Seed mock symbol
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
                    ident: ":symbol/namespace".into(),
                    v: MesoValue::String("core".into()),
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
        // Since we seeded ":symbol/namespace" as "core", we expect the package to be "core"
        assert!(rust_lang.packages.contains_key("core"));
    }
}
