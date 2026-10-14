// lena-core/src/docgen.rs

use crate::db::LenaDb;
use minijinja::{Environment, context};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocGenError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Template error: {0}")]
    Template(String),
}

#[derive(Debug, Serialize, Clone)]
pub struct DocEntity {
    pub fqsn: String,
    pub name: String,
    pub kind: String,
    pub language: String,
    pub signature: String,
    pub docs: String,
    pub file: String,
    pub line: i64,
}

const DEFAULT_TEMPLATE: &str = include_str!("../templates/index.html");
const MDBOOK_TEMPLATE: &str = include_str!("../templates/mdbook.html");

pub async fn generate_static_docs(
    db: &Arc<LenaDb>,
    output_dir: &Path,
    theme: &str,
) -> Result<(), DocGenError> {
    let query = "[:find (pull ?e [*]) :where [?e :location/file ?file]]";

    let json_result = db
        .inner()
        .query_json(query)
        .await
        .map_err(|e| DocGenError::Database(e.to_string()))?;

    let rows: Vec<Value> = serde_json::from_str(&json_result).unwrap_or_default();
    let mut entities = Vec::new();

    for row in rows {
        let data = row.get("e").unwrap_or(&row);

        let kind_str = extract_string(data, "entity/type");
        if kind_str == ":type/call-site" || kind_str.is_empty() {
            continue;
        }

        let fqsn = extract_string(data, "symbol/fqsn");
        let name = extract_string(data, "symbol/name");
        let language = extract_string(data, "entity/language").replace(":lang/", "");
        let signature = extract_string(data, "code/signature").replace("\\n", "\n");
        let docs = extract_string(data, "doc/text").replace("\\n", "\n");
        let file = extract_string(data, "location/file");
        let line = extract_i64(data, "location/start-line");

        entities.push(DocEntity {
            fqsn,
            name: if name.is_empty() {
                "Unnamed".to_string()
            } else {
                name
            },
            kind: kind_str.replace(":type/", ""),
            language,
            signature,
            docs,
            file,
            line,
        });
    }

    entities.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    let mut grouped: HashMap<String, HashMap<String, Vec<DocEntity>>> = HashMap::new();
    for e in entities {
        grouped
            .entry(e.language.clone())
            .or_default()
            .entry(e.file.clone())
            .or_default()
            .push(e);
    }

    let template_str = match theme {
        "mdbook" | "book" => MDBOOK_TEMPLATE,
        _ => DEFAULT_TEMPLATE,
    };

    let mut env = Environment::new();
    env.add_template("index.html", template_str)
        .map_err(|e| DocGenError::Template(e.to_string()))?;

    let tmpl = env
        .get_template("index.html")
        .map_err(|e| DocGenError::Template(e.to_string()))?;

    let rendered = tmpl
        .render(context!(grouped => grouped))
        .map_err(|e| DocGenError::Template(e.to_string()))?;

    fs::create_dir_all(output_dir)?;
    fs::write(output_dir.join("index.html"), rendered)?;

    Ok(())
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
