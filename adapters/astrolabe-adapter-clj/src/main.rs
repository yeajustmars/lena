// adapaters/astrolabe-adapter-clj/src/main.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use astrolabe_protocol::emitter::{EdnValue, TransactionBuilder};
use astrolabe_protocol::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_DOC_TEXT, ATTR_ENTITY_LANGUAGE, ATTR_ENTITY_TYPE,
    ATTR_LOC_END_COL, ATTR_LOC_END_LINE, ATTR_LOC_FILE, ATTR_LOC_START_COL, ATTR_LOC_START_LINE,
    ATTR_PARENT_ENTITY, ATTR_SYMBOL_FQSN, ATTR_SYMBOL_KIND, ATTR_SYMBOL_NAME,
    ATTR_SYMBOL_NAMESPACE, KIND_FN, LANG_CLOJURE, TYPE_CALL_SITE, TYPE_FUNCTION, TYPE_MODULE,
};
use clap::Parser;
use serde::Deserialize;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
    process::Command,
};

#[derive(Parser)]
#[command(name = "astrolabe-adapter-clj")]
#[command(about = "Clojure adapter for Astrolabe using clj-kondo")]
struct Cli {
    /// Target directory to analyze
    #[arg(default_value = ".")]
    target: PathBuf,
}

#[derive(Deserialize, Debug)]
struct KondoOutput {
    analysis: Analysis,
}

#[derive(Deserialize, Debug)]
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
    row: i64,
    col: i64,
}

#[derive(Deserialize, Debug)]
struct VarDef {
    ns: String,
    name: String,
    filename: String,
    row: i64,
    col: i64,
    #[serde(rename = "end-row")]
    end_row: Option<i64>,
    #[serde(rename = "end-col")]
    end_col: Option<i64>,
    doc: Option<String>,
}

#[derive(Deserialize, Debug)]
struct VarUsage {
    from: Option<String>,
    #[serde(rename = "from-var")]
    from_var: Option<String>,
    to: Option<String>,
    name: String,
    filename: String,
    row: i64,
    col: i64,
}

fn hash_id(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let output = Command::new("clj-kondo")
        .arg("--lint")
        .arg(&cli.target)
        .arg("--config")
        // Pass valid EDN here instead of JSON!
        .arg("{:output {:format :json, :analysis true}}")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("clj-kondo failed: {stderr}");
    }

    let kondo_data: KondoOutput = serde_json::from_slice(&output.stdout)?;

    if let Some(namespaces) = kondo_data.analysis.namespace_definitions {
        for ns in namespaces {
            let mut tx = TransactionBuilder::new();
            let fqsn = format!("clj:{}", ns.name);
            let e_id = hash_id(&fqsn);

            tx.add(e_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_MODULE))?;
            tx.add(e_id, ATTR_ENTITY_LANGUAGE, EdnValue::Keyword(LANG_CLOJURE))?;
            tx.add(e_id, ATTR_SYMBOL_FQSN, EdnValue::String(&fqsn))?;
            tx.add(e_id, ATTR_SYMBOL_NAME, EdnValue::String(&ns.name))?;
            tx.add(e_id, ATTR_LOC_FILE, EdnValue::String(&ns.filename))?;
            tx.add(e_id, ATTR_LOC_START_LINE, EdnValue::Int(ns.row))?;
            tx.add(e_id, ATTR_LOC_START_COL, EdnValue::Int(ns.col))?;

            println!("{}", tx.finish());
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
            tx.add(e_id, ATTR_SYMBOL_NAMESPACE, EdnValue::String(&v.ns))?;
            tx.add(e_id, ATTR_SYMBOL_NAME, EdnValue::String(&v.name))?;
            tx.add(e_id, ATTR_SYMBOL_KIND, EdnValue::Keyword(KIND_FN))?;
            tx.add(e_id, ATTR_PARENT_ENTITY, EdnValue::Ref(parent_id))?;
            tx.add(e_id, ATTR_LOC_FILE, EdnValue::String(&v.filename))?;
            tx.add(e_id, ATTR_LOC_START_LINE, EdnValue::Int(v.row))?;
            tx.add(e_id, ATTR_LOC_START_COL, EdnValue::Int(v.col))?;

            if let Some(end_row) = v.end_row {
                tx.add(e_id, ATTR_LOC_END_LINE, EdnValue::Int(end_row))?;
            }
            if let Some(end_col) = v.end_col {
                tx.add(e_id, ATTR_LOC_END_COL, EdnValue::Int(end_col))?;
            }
            if let Some(doc) = v.doc {
                tx.add(e_id, ATTR_DOC_TEXT, EdnValue::String(&doc))?;
            }

            println!("{}", tx.finish());
        }
    }

    if let Some(usages) = kondo_data.analysis.var_usages {
        for u in usages {
            if let (Some(from_ns), Some(from_var), Some(to_ns)) = (u.from, u.from_var, u.to) {
                let mut tx = TransactionBuilder::new();
                let caller_fqsn = format!("clj:{from_ns}/{from_var}");
                let callee_fqsn = format!("clj:{to_ns}/{}", u.name);

                let caller_id = hash_id(&caller_fqsn);
                let callee_id = hash_id(&callee_fqsn);

                let call_site_id = hash_id(&format!(
                    "{}:{}:{}:{caller_fqsn}->{callee_fqsn}",
                    u.filename, u.row, u.col
                ));

                tx.add(
                    call_site_id,
                    ATTR_ENTITY_TYPE,
                    EdnValue::Keyword(TYPE_CALL_SITE),
                )?;
                tx.add(call_site_id, ATTR_CALL_CALLER, EdnValue::Ref(caller_id))?;
                tx.add(call_site_id, ATTR_CALL_CALLEE, EdnValue::Ref(callee_id))?;
                tx.add(call_site_id, ATTR_LOC_FILE, EdnValue::String(&u.filename))?;
                tx.add(call_site_id, ATTR_LOC_START_LINE, EdnValue::Int(u.row))?;
                tx.add(call_site_id, ATTR_LOC_START_COL, EdnValue::Int(u.col))?;

                println!("{}", tx.finish());
            }
        }
    }

    Ok(())
}
