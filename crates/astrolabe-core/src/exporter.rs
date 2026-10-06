//! Prompt Exporting Engine

use crate::db::AstrolabeDb;
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExporterError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Entity not found: {0}")]
    NotFound(String),
    #[error("Failed to parse entity data: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Xml,
    Markdown,
}

/// Helper to extract a string from either `:key` or `key` inside a JSON Value.
fn extract_string(val: &Value, key: &str) -> String {
    let key_with_colon = if key.starts_with(':') {
        key.to_string()
    } else {
        format!(":{key}")
    };
    let key_without_colon = key.trim_start_matches(':');

    val.get(&key_with_colon)
        .or_else(|| val.get(key_without_colon))
        .and_then(|v| {
            v.as_str().map(ToString::to_string).or_else(|| {
                v.as_array()
                    .and_then(|arr| arr.first()?.as_str().map(ToString::to_string))
            })
        })
        .unwrap_or_default()
}

/// Helper to extract an integer from either `:key` or `key` inside a JSON Value.
fn extract_i64(val: &Value, key: &str) -> i64 {
    let key_with_colon = if key.starts_with(':') {
        key.to_string()
    } else {
        format!(":{key}")
    };
    let key_without_colon = key.trim_start_matches(':');

    val.get(&key_with_colon)
        .or_else(|| val.get(key_without_colon))
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_array().and_then(|arr| arr.first()?.as_i64()))
        })
        .unwrap_or(0)
}

/// Retrieves an entity by its FQSN and formats it for LLM context.
pub async fn export_prompt(
    db: &Arc<AstrolabeDb>,
    fqsn: &str,
    format: ExportFormat,
) -> Result<String, ExporterError> {
    let query = format!("[:find (pull ?e [*]) :where [?e :symbol/fqsn \"{fqsn}\"]]");

    let json_result = db
        .inner()
        .query_json(&query)
        .await
        .map_err(|e| ExporterError::Database(e.to_string()))?;

    let rows: Vec<Value> =
        serde_json::from_str(&json_result).map_err(|e| ExporterError::Parse(e.to_string()))?;

    if rows.is_empty() {
        return Err(ExporterError::NotFound(fqsn.to_string()));
    }

    // MesoDB returns rows as `[{"e": {":symbol/name": "add", ...}}]`
    let entity_data = rows[0].get("e").cloned().unwrap_or_else(|| rows[0].clone());

    let name = extract_string(&entity_data, "symbol/name");
    let lang = extract_string(&entity_data, "entity/language");
    let file = extract_string(&entity_data, "location/file");
    let line = extract_i64(&entity_data, "location/start-line");
    let sig = extract_string(&entity_data, "code/signature");
    let docs = extract_string(&entity_data, "doc/text");

    let name = if name.is_empty() { "Unknown" } else { &name };
    let lang = if lang.is_empty() { "Unknown" } else { &lang };
    let file = if file.is_empty() { "Unknown" } else { &file };

    match format {
        ExportFormat::Xml => Ok(format_xml(name, lang, file, line, &sig, &docs)),
        ExportFormat::Markdown => Ok(format_markdown(name, lang, file, line, &sig, &docs)),
    }
}

fn format_xml(name: &str, lang: &str, file: &str, line: i64, sig: &str, docs: &str) -> String {
    let mut out = format!("<entity name=\"{name}\" language=\"{lang}\">\n");
    out.push_str(&format!("  <location file=\"{file}\" line=\"{line}\" />\n"));

    if !sig.is_empty() {
        out.push_str("  <signature>\n");
        out.push_str(&format!("    <![CDATA[\n{sig}\n    ]]>\n"));
        out.push_str("  </signature>\n");
    }
    if !docs.is_empty() {
        out.push_str("  <documentation>\n");
        out.push_str(&format!("    <![CDATA[\n{docs}\n    ]]>\n"));
        out.push_str("  </documentation>\n");
    }

    out.push_str("</entity>");
    out
}

fn format_markdown(name: &str, lang: &str, file: &str, line: i64, sig: &str, docs: &str) -> String {
    let lang_clean = lang.replace(":lang/", "");
    let mut out = format!("### `{name}`\n\n");
    out.push_str(&format!("- **Language:** `{lang_clean}`\n"));
    out.push_str(&format!("- **Location:** `{file}:{line}`\n\n"));

    if !sig.is_empty() {
        out.push_str("**Signature:**\n");
        out.push_str(&format!("```{}\n{sig}\n```\n\n", lang_clean));
    }
    if !docs.is_empty() {
        out.push_str("**Documentation:**\n");
        out.push_str(&format!("```text\n{docs}\n```\n"));
    }

    out.trim_end().to_string()
}
