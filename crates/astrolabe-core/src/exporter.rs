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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AstrolabeDb;
    use mesodb_core::transactor::Fact;
    use mesodb_core::types::Value as MesoValue;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_extract_string_with_and_without_colons() {
        let val1 = json!({ ":symbol/name": "test_func" });
        let val2 = json!({ "symbol/name": "test_func_no_colon" });
        let val3 = json!({ "symbol/name": ["array_string"] });

        assert_eq!(extract_string(&val1, "symbol/name"), "test_func");
        assert_eq!(extract_string(&val2, ":symbol/name"), "test_func_no_colon");
        assert_eq!(extract_string(&val3, "symbol/name"), "array_string");
        assert_eq!(extract_string(&val1, "missing_key"), "");
    }

    #[test]
    fn test_extract_i64_with_and_without_colons() {
        let val1 = json!({ ":location/start-line": 42 });
        let val2 = json!({ "location/start-line": 99 });
        let val3 = json!({ "location/start-line": [7] });

        assert_eq!(extract_i64(&val1, "location/start-line"), 42);
        assert_eq!(extract_i64(&val2, ":location/start-line"), 99);
        assert_eq!(extract_i64(&val3, "location/start-line"), 7);
        assert_eq!(extract_i64(&val1, "missing_key"), 0);
    }

    #[test]
    fn test_format_xml() {
        let xml = format_xml(
            "add",
            "Rust",
            "src/lib.rs",
            10,
            "fn add(a: i32) -> i32",
            "Adds numbers",
        );
        assert!(xml.contains("<entity name=\"add\" language=\"Rust\">"));
        assert!(xml.contains("<location file=\"src/lib.rs\" line=\"10\" />"));
        assert!(xml.contains("<![CDATA[\nfn add(a: i32) -> i32\n    ]]>"));
        assert!(xml.contains("<![CDATA[\nAdds numbers\n    ]]>"));
    }

    #[test]
    fn test_format_markdown() {
        let md = format_markdown(
            "add",
            ":lang/Rust",
            "src/lib.rs",
            10,
            "fn add(a: i32)",
            "Docs here",
        );
        assert!(md.contains("### `add`"));
        assert!(md.contains("- **Language:** `Rust`")); // Should strip :lang/
        assert!(md.contains("- **Location:** `src/lib.rs:10`"));
        assert!(md.contains("```Rust\nfn add(a: i32)\n```"));
        assert!(md.contains("```text\nDocs here\n```"));
    }

    #[tokio::test]
    async fn test_export_prompt_not_found() {
        let dir = tempdir().unwrap();
        let db = Arc::new(AstrolabeDb::open(dir.path().join("not_found.db")).unwrap());

        let res = export_prompt(&db, "rust:missing::func", ExportFormat::Xml).await;
        assert!(matches!(res, Err(ExporterError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_export_prompt_success() {
        let dir = tempdir().unwrap();
        let db = Arc::new(AstrolabeDb::open(dir.path().join("export_success.db")).unwrap());

        // Transact the exact schema attributes the exporter looks for
        db.inner()
            .transact(vec![
                Fact {
                    e: 1,
                    ident: ":symbol/fqsn".into(),
                    v: MesoValue::String("rust:test::success".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":symbol/name".into(),
                    v: MesoValue::String("success_func".into()),
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
                    ident: ":location/file".into(),
                    v: MesoValue::String("src/main.rs".into()),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
                Fact {
                    e: 1,
                    ident: ":location/start-line".into(),
                    v: MesoValue::Int64(42),
                    op: true,
                    cas_old_v: None,
                    valid_time: None,
                },
            ])
            .await
            .unwrap();

        let xml_res = export_prompt(&db, "rust:test::success", ExportFormat::Xml)
            .await
            .unwrap();
        assert!(xml_res.contains("name=\"success_func\""));
        assert!(xml_res.contains("language=\"Rust\""));
        assert!(xml_res.contains("line=\"42\""));
    }
}
