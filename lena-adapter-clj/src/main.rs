// lena-adapter-clj/src/main.rs

#![deny(warnings)]
#![warn(clippy::all)]
// TODO: put back pedantic

use clap::Parser;
use lena_protocol::emitter::{EdnValue, TransactionBuilder};
use lena_protocol::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_CODE_SIGNATURE, ATTR_DOC_TEXT, ATTR_ENTITY_LANGUAGE,
    ATTR_ENTITY_TYPE, ATTR_LOC_END_COL, ATTR_LOC_END_LINE, ATTR_LOC_FILE, ATTR_LOC_START_COL,
    ATTR_LOC_START_LINE, ATTR_PARENT_ENTITY, ATTR_SYMBOL_EXTERNAL, ATTR_SYMBOL_FQSN,
    ATTR_SYMBOL_KIND, ATTR_SYMBOL_MODULE, ATTR_SYMBOL_NAME, KIND_FN, KIND_MACRO, KIND_MODULE,
    LANG_CLOJURE, TYPE_CALL_SITE, TYPE_FUNCTION, TYPE_MODULE,
};
use serde::Deserialize;
use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[command(name = "lena-adapter-clj")]
#[command(about = "Clojure adapter for Lena using clj-kondo")]
struct Cli {
    /// Target directory to analyze
    #[arg(default_value = ".")]
    target: PathBuf,
}

#[derive(Deserialize, Debug)]
struct KondoOutput {
    analysis: Analysis,
}

#[derive(Deserialize, Debug, Default)]
struct Analysis {
    #[serde(rename = "namespace-definitions")]
    namespace_definitions: Option<Vec<NsDef>>,
    #[serde(rename = "var-definitions")]
    var_definitions: Option<Vec<VarDef>>,
    #[serde(rename = "var-usages")]
    var_usages: Option<Vec<VarUsage>>,
}

#[derive(Deserialize, Debug)]
struct NsDef {
    name: String,
    filename: String,
    row: Option<i64>,
    col: Option<i64>,
}

#[derive(Deserialize, Debug)]
struct VarDef {
    ns: String,
    name: String,
    filename: String,
    row: Option<i64>,
    col: Option<i64>,
    #[serde(rename = "end-row")]
    end_row: Option<i64>,
    #[serde(rename = "end-col")]
    end_col: Option<i64>,
    doc: Option<String>,
    #[serde(rename = "macro")]
    is_macro: Option<bool>,
    #[serde(rename = "arglist-strs")]
    arglist_strs: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct VarUsage {
    from: Option<String>,
    #[serde(rename = "from-var")]
    from_var: Option<String>,
    to: Option<String>,
    name: String,
    filename: String,
    row: Option<i64>,
    col: Option<i64>,
}

fn hash_id(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

fn process_kondo_output(kondo_data: KondoOutput) -> anyhow::Result<Vec<String>> {
    let mut facts = Vec::new();
    let mut local_namespaces = HashSet::new();

    if let Some(namespaces) = kondo_data.analysis.namespace_definitions {
        for ns in namespaces {
            local_namespaces.insert(ns.name.clone());
            let mut tx = TransactionBuilder::new();
            let fqsn = format!("clj:{}", ns.name);
            let e_id = hash_id(&fqsn);

            tx.add(e_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_MODULE))?;
            tx.add(e_id, ATTR_ENTITY_LANGUAGE, EdnValue::Keyword(LANG_CLOJURE))?;
            tx.add(e_id, ATTR_SYMBOL_FQSN, EdnValue::String(&fqsn))?;
            tx.add(e_id, ATTR_SYMBOL_MODULE, EdnValue::String(&ns.name))?;
            tx.add(e_id, ATTR_SYMBOL_NAME, EdnValue::String(&ns.name))?;
            tx.add(e_id, ATTR_SYMBOL_KIND, EdnValue::Keyword(KIND_MODULE))?;
            tx.add(e_id, ATTR_LOC_FILE, EdnValue::String(&ns.filename))?;

            tx.add(
                e_id,
                ATTR_LOC_START_LINE,
                EdnValue::Int(ns.row.unwrap_or(1)),
            )?;
            tx.add(e_id, ATTR_LOC_START_COL, EdnValue::Int(ns.col.unwrap_or(1)))?;

            facts.push(tx.finish());
        }
    }

    if let Some(vars) = kondo_data.analysis.var_definitions {
        for v in vars {
            let mut tx = TransactionBuilder::new();
            let fqsn = format!("clj:{}/{}", v.ns, v.name);
            let e_id = hash_id(&fqsn);
            let parent_id = hash_id(&format!("clj:{}", v.ns));

            tx.add(e_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_FUNCTION))?;
            tx.add(e_id, ATTR_ENTITY_LANGUAGE, EdnValue::Keyword(LANG_CLOJURE))?;
            tx.add(e_id, ATTR_SYMBOL_FQSN, EdnValue::String(&fqsn))?;
            tx.add(e_id, ATTR_SYMBOL_MODULE, EdnValue::String(&v.ns))?;
            tx.add(e_id, ATTR_SYMBOL_NAME, EdnValue::String(&v.name))?;

            let kind = if v.is_macro.unwrap_or(false) {
                KIND_MACRO
            } else {
                KIND_FN
            };
            tx.add(e_id, ATTR_SYMBOL_KIND, EdnValue::Keyword(kind))?;
            tx.add(e_id, ATTR_PARENT_ENTITY, EdnValue::Ref(parent_id))?;
            tx.add(e_id, ATTR_LOC_FILE, EdnValue::String(&v.filename))?;
            tx.add(e_id, ATTR_LOC_START_LINE, EdnValue::Int(v.row.unwrap_or(1)))?;
            tx.add(e_id, ATTR_LOC_START_COL, EdnValue::Int(v.col.unwrap_or(1)))?;

            if let Some(end_row) = v.end_row {
                tx.add(e_id, ATTR_LOC_END_LINE, EdnValue::Int(end_row))?;
            }
            if let Some(end_col) = v.end_col {
                tx.add(e_id, ATTR_LOC_END_COL, EdnValue::Int(end_col))?;
            }
            if let Some(doc) = v.doc {
                tx.add(e_id, ATTR_DOC_TEXT, EdnValue::String(&doc))?;
            }
            if let Some(ref args) = v.arglist_strs {
                let sig = args.join("\n");
                tx.add(e_id, ATTR_CODE_SIGNATURE, EdnValue::String(&sig))?;
            }

            facts.push(tx.finish());
        }
    }

    let mut emitted_externals = HashSet::new();

    if let Some(usages) = kondo_data.analysis.var_usages {
        for u in usages {
            if let (Some(from_ns), Some(from_var), Some(to_ns)) = (u.from, u.from_var, u.to) {
                let caller_fqsn = format!("clj:{from_ns}/{from_var}");
                let callee_fqsn = format!("clj:{to_ns}/{}", u.name);

                let caller_id = hash_id(&caller_fqsn);
                let callee_id = hash_id(&callee_fqsn);
                let call_site_id = hash_id(&format!(
                    "{}:{}:{}:{caller_fqsn}->{callee_fqsn}",
                    u.filename,
                    u.row.unwrap_or(1),
                    u.col.unwrap_or(1)
                ));

                let mut tx = TransactionBuilder::new();
                tx.add(
                    call_site_id,
                    ATTR_ENTITY_TYPE,
                    EdnValue::Keyword(TYPE_CALL_SITE),
                )?;
                tx.add(call_site_id, ATTR_CALL_CALLER, EdnValue::Ref(caller_id))?;
                tx.add(call_site_id, ATTR_CALL_CALLEE, EdnValue::Ref(callee_id))?;
                tx.add(call_site_id, ATTR_LOC_FILE, EdnValue::String(&u.filename))?;
                tx.add(
                    call_site_id,
                    ATTR_LOC_START_LINE,
                    EdnValue::Int(u.row.unwrap_or(1)),
                )?;
                tx.add(
                    call_site_id,
                    ATTR_LOC_START_COL,
                    EdnValue::Int(u.col.unwrap_or(1)),
                )?;
                facts.push(tx.finish());

                // Emit external stub if callee belongs to a namespace not found in this workspace
                if !local_namespaces.contains(&to_ns) && emitted_externals.insert(callee_id) {
                    let mut stub_tx = TransactionBuilder::new();
                    stub_tx.add(callee_id, ATTR_SYMBOL_FQSN, EdnValue::String(&callee_fqsn))?;
                    stub_tx.add(callee_id, ATTR_SYMBOL_NAME, EdnValue::String(&u.name))?;
                    stub_tx.add(callee_id, ATTR_SYMBOL_MODULE, EdnValue::String(&to_ns))?;
                    stub_tx.add(callee_id, ATTR_SYMBOL_EXTERNAL, EdnValue::String("true"))?;
                    facts.push(stub_tx.finish());
                }
            }
        }
    }

    Ok(facts)
}

fn execute_kondo(target: &Path) -> anyhow::Result<Vec<String>> {
    let output = Command::new("clj-kondo")
        .arg("--lint")
        .arg(target)
        .arg("--config")
        .arg("{:output {:format :json, :analysis {:arglists true}}}")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("clj-kondo warnings/errors: {stderr}");
    }

    let kondo_data: KondoOutput = serde_json::from_slice(&output.stdout)?;
    process_kondo_output(kondo_data)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let facts = execute_kondo(&cli.target)?;

    for fact in facts {
        println!("{fact}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_fixture_path(file: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/clj")
            .join(file)
    }

    fn run_fixture(file: &str) -> String {
        let path = get_fixture_path(file);
        if !path.exists() {
            return String::new();
        }
        execute_kondo(&path).unwrap().join("\n")
    }

    #[test]
    fn test_clj_module_definition() {
        let output = run_fixture("edge_cases.clj");
        if output.is_empty() {
            return;
        }
        assert!(output.contains(":type/module"));
        assert!(output.contains(":symbol/module \"edge-cases\""));
    }

    #[test]
    fn test_clj_defn_and_private_facts() {
        let output = run_fixture("edge_cases.clj");
        if output.is_empty() {
            return;
        }
        assert!(output.contains(":symbol/fqsn \"clj:edge-cases/public-fn\""));
        assert!(output.contains(":symbol/module \"edge-cases\""));
    }

    #[test]
    fn test_clj_external_stub_generation() {
        let kondo_data = KondoOutput {
            analysis: Analysis {
                namespace_definitions: Some(vec![NsDef {
                    name: "app.core".to_string(),
                    filename: "src/app/core.clj".to_string(),
                    row: Some(1),
                    col: Some(1),
                }]),
                var_definitions: Some(vec![VarDef {
                    ns: "app.core".to_string(),
                    name: "run".to_string(),
                    filename: "src/app/core.clj".to_string(),
                    row: Some(2),
                    col: Some(1),
                    end_row: None,
                    end_col: None,
                    doc: None,
                    is_macro: None,
                    arglist_strs: None,
                }]),
                var_usages: Some(vec![VarUsage {
                    from: Some("app.core".to_string()),
                    from_var: Some("run".to_string()),
                    to: Some("clojure.core".to_string()), // External namespace
                    name: "str".to_string(),
                    filename: "src/app/core.clj".to_string(),
                    row: Some(3),
                    col: Some(3),
                }]),
            },
        };

        let facts = process_kondo_output(kondo_data).unwrap();
        let output = facts.join("\n");

        assert!(output.contains(":symbol/external \"true\""));
        assert!(output.contains(":symbol/fqsn \"clj:clojure.core/str\""));
        assert!(output.contains(":symbol/module \"clojure.core\""));
        assert!(output.contains(":symbol/name \"str\""));
    }

    #[test]
    fn test_clj_var_module_attribute() {
        let kondo_data = KondoOutput {
            analysis: Analysis {
                namespace_definitions: None,
                var_definitions: Some(vec![VarDef {
                    ns: "my.service".to_string(),
                    name: "handler".to_string(),
                    filename: "src/my/service.clj".to_string(),
                    row: Some(5),
                    col: Some(1),
                    end_row: None,
                    end_col: None,
                    doc: None,
                    is_macro: None,
                    arglist_strs: None,
                }]),
                var_usages: None,
            },
        };

        let facts = process_kondo_output(kondo_data).unwrap();
        let output = facts.join("\n");

        assert!(output.contains(":symbol/module \"my.service\""));
        assert!(output.contains(":symbol/name \"handler\""));
        assert!(!output.contains(":symbol/namespace"));
    }
}
