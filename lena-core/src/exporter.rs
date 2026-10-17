//! Prompt Exporting Engine

use crate::db::LenaDb;
use minijinja::Environment;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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
    #[error("Template rendering error: {0}")]
    Template(String),
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Xml,
    Markdown,
    Template(String),
}

/// The Canonical Context Intermediate Representation (IR).
/// This decouples graph extraction from view rendering.
#[derive(Debug, Clone, Serialize)]
pub struct LenaContext {
    pub fqsn: String,
    pub name: String,
    pub language: String,
    pub location: SourceLocation,
    pub payload: Payload,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Payload {
    pub signature: String,
    pub docs: String,
}

impl LenaContext {
    #[must_use]
    pub fn to_xml(&self) -> String {
        let mut out = format!(
            "<entity name=\"{}\" language=\"{}\">\n",
            self.name, self.language
        );

        out.push_str(&format!(
            "  <location file=\"{}\" line=\"{}\" />\n",
            self.location.file, self.location.line
        ));

        if !self.payload.signature.is_empty() {
            out.push_str("  <signature>\n");
            out.push_str(&format!(
                "    <![CDATA[\n{}\n    ]]>\n",
                self.payload.signature
            ));
            out.push_str("  </signature>\n");
        }
        if !self.payload.docs.is_empty() {
            out.push_str("  <documentation>\n");
            out.push_str(&format!("    <![CDATA[\n{}\n    ]]>\n", self.payload.docs));
            out.push_str("  </documentation>\n");
        }
        if !self.callers.is_empty() {
            out.push_str("  <callers>\n");
            for caller in &self.callers {
                out.push_str(&format!("    <caller fqsn=\"{caller}\" />\n"));
            }
            out.push_str("  </callers>\n");
        }
        if !self.callees.is_empty() {
            out.push_str("  <callees>\n");
            for callee in &self.callees {
                out.push_str(&format!("    <callee fqsn=\"{callee}\" />\n"));
            }
            out.push_str("  </callees>\n");
        }

        out.push_str("</entity>");
        out
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let lang_clean = self.language.replace(":lang/", "");
        let mut out = format!("### `{}`\n\n", self.name);
        out.push_str(&format!("- **Language:** `{lang_clean}`\n"));
        out.push_str(&format!(
            "- **Location:** `{}:{}`\n\n",
            self.location.file, self.location.line
        ));

        if !self.payload.signature.is_empty() {
            out.push_str("**Signature:**\n");
            out.push_str(&format!(
                "```{}\n{}\n```\n\n",
                lang_clean.to_lowercase(),
                self.payload.signature
            ));
        }
        if !self.payload.docs.is_empty() {
            out.push_str("**Documentation:**\n");
            out.push_str(&format!("```text\n{}\n```\n\n", self.payload.docs));
        }
        if !self.callers.is_empty() {
            out.push_str("**Called By:**\n");
            for caller in &self.callers {
                out.push_str(&format!("- `{caller}`\n"));
            }
            out.push('\n');
        }
        if !self.callees.is_empty() {
            out.push_str("**Makes Calls To:**\n");
            for callee in &self.callees {
                out.push_str(&format!("- `{callee}`\n"));
            }
            out.push('\n');
        }

        out.trim_end().to_string()
    }
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
    db: &Arc<LenaDb>,
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

    let raw_name = extract_string(&entity_data, "symbol/name");
    let raw_lang = extract_string(&entity_data, "entity/language");
    let raw_file = extract_string(&entity_data, "location/file");

    let name = if raw_name.is_empty() {
        "Unknown".to_string()
    } else {
        raw_name
    };
    let language = if raw_lang.is_empty() {
        "Unknown".to_string()
    } else {
        raw_lang
    };
    let file = if raw_file.is_empty() {
        "Unknown".to_string()
    } else {
        raw_file
    };
    let line = extract_i64(&entity_data, "location/start-line");
    let signature = extract_string(&entity_data, "code/signature");
    let docs = extract_string(&entity_data, "doc/text");

    // Fetch callers
    let callers_query = format!(
        "[:find ?caller_fqsn :where [?callee :symbol/fqsn \"{fqsn}\"] (calls_rule ?caller ?callee) [?caller :symbol/fqsn ?caller_fqsn]]"
    );
    let mut callers = Vec::new();
    if let Ok(caller_json) = db.query_with_rules(&callers_query).await
        && let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&caller_json)
    {
        for row in rows {
            let caller = if let Some(arr) = row.as_array() {
                arr.first().and_then(|v| v.as_str())
            } else if let Some(obj) = row.as_object() {
                obj.values().next().and_then(|v| v.as_str())
            } else {
                None
            };
            if let Some(caller_str) = caller {
                callers.push(caller_str.to_string());
            }
        }
    }

    // Fetch callees
    let callees_query = format!(
        "[:find ?callee_fqsn :where [?caller :symbol/fqsn \"{fqsn}\"] (calls_rule ?caller ?callee) [?callee :symbol/fqsn ?callee_fqsn]]"
    );
    let mut callees = Vec::new();
    if let Ok(callee_json) = db.query_with_rules(&callees_query).await
        && let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&callee_json)
    {
        for row in rows {
            let callee = if let Some(arr) = row.as_array() {
                arr.first().and_then(|v| v.as_str())
            } else if let Some(obj) = row.as_object() {
                obj.values().next().and_then(|v| v.as_str())
            } else {
                None
            };
            if let Some(callee_str) = callee {
                callees.push(callee_str.to_string());
            }
        }
    }

    callers.sort();
    callers.dedup();
    callees.sort();
    callees.dedup();

    let context = LenaContext {
        fqsn: fqsn.to_string(),
        name,
        language,
        location: SourceLocation { file, line },
        payload: Payload { signature, docs },
        callers,
        callees,
    };

    match format {
        ExportFormat::Xml => Ok(context.to_xml()),
        ExportFormat::Markdown => Ok(context.to_markdown()),
        ExportFormat::Template(tpl_str) => {
            let mut env = Environment::new();
            env.add_template("prompt", &tpl_str)
                .map_err(|e| ExporterError::Template(e.to_string()))?;
            let tmpl = env
                .get_template("prompt")
                .map_err(|e| ExporterError::Template(e.to_string()))?;
            tmpl.render(&context)
                .map_err(|e| ExporterError::Template(e.to_string()))
        }
    }
}

/// Exports an entire directory into a formatted string (Markdown or XML).
pub fn export_directory(dir: &Path, format: ExportFormat) -> Result<String, ExporterError> {
    let mut files = Vec::new();
    collect_files(dir, &mut files).map_err(|e| ExporterError::Parse(e.to_string()))?;

    // Sort to ensure deterministic output
    files.sort();

    let mut out = String::new();

    match format {
        ExportFormat::Xml => {
            out.push_str(&format!("<directory path=\"{}\">\n", dir.display()));
            for file in files {
                if let Ok(content) = fs::read_to_string(&file) {
                    out.push_str(&format!("  <file path=\"{}\">\n", file.display()));
                    out.push_str(&format!("    <![CDATA[\n{}\n    ]]>\n", content));
                    out.push_str("  </file>\n");
                }
            }
            out.push_str("</directory>");
        }
        ExportFormat::Markdown => {
            out.push_str(&format!("Project Path: {}\n\n", dir.display()));
            for file in files {
                if let Ok(content) = fs::read_to_string(&file) {
                    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("txt");
                    out.push_str(&format!("`{}`:\n", file.display()));
                    out.push_str(&format!("```{}\n{}\n```\n\n", ext, content));
                }
            }
        }
        ExportFormat::Template(_) => {
            return Err(ExporterError::Template(
                "Templates not supported for directory exports".into(),
            ));
        }
    }

    Ok(out.trim_end().to_string())
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        if dir.is_file() {
            files.push(dir.to_path_buf());
        }
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Ignore build/cache directories, hidden folders, and lockfiles
            if name == "target"
                || name == ".git"
                || name == "dist"
                || name == ".lena"
                || name == "node_modules"
                || name == "Cargo.lock"
            {
                continue;
            }
        }

        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            // Only add files that can be read as UTF-8 text (skip binary files if possible,
            // though read_to_string later will handle the safety check)
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::LenaDb;
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
        let ctx = LenaContext {
            fqsn: "rust:crate::add".to_string(),
            name: "add".to_string(),
            language: "Rust".to_string(),
            location: SourceLocation {
                file: "src/lib.rs".to_string(),
                line: 10,
            },
            payload: Payload {
                signature: "fn add(a: i32) -> i32".to_string(),
                docs: "Adds numbers".to_string(),
            },
            callers: vec!["rust:crate::main".to_string()],
            callees: vec!["rust:std::ops::Add".to_string()],
        };

        let xml = ctx.to_xml();
        assert!(xml.contains("<entity name=\"add\" language=\"Rust\">"));
        assert!(xml.contains("<location file=\"src/lib.rs\" line=\"10\" />"));
        assert!(xml.contains("<![CDATA[\nfn add(a: i32) -> i32\n    ]]>"));
        assert!(xml.contains("<![CDATA[\nAdds numbers\n    ]]>"));
        assert!(xml.contains("<caller fqsn=\"rust:crate::main\" />"));
        assert!(xml.contains("<callee fqsn=\"rust:std::ops::Add\" />"));
    }

    #[test]
    fn test_format_markdown() {
        let ctx = LenaContext {
            fqsn: "rust:crate::add".to_string(),
            name: "add".to_string(),
            language: ":lang/Rust".to_string(),
            location: SourceLocation {
                file: "src/lib.rs".to_string(),
                line: 10,
            },
            payload: Payload {
                signature: "fn add(a: i32)".to_string(),
                docs: "Docs here".to_string(),
            },
            callers: vec!["rust:crate::main".to_string()],
            callees: vec![],
        };

        let md = ctx.to_markdown();
        assert!(md.contains("### `add`"));
        assert!(md.contains("- **Language:** `Rust`"));
        assert!(md.contains("- **Location:** `src/lib.rs:10`"));
        assert!(md.contains("```rust\nfn add(a: i32)\n```"));
        assert!(md.contains("```text\nDocs here\n```"));
        assert!(md.contains("**Called By:**\n- `rust:crate::main`"));
        assert!(!md.contains("**Makes Calls To:**"));
    }

    #[tokio::test]
    async fn test_export_prompt_not_found() {
        let dir = tempdir().unwrap();
        let db = Arc::new(LenaDb::open(dir.path().join("not_found.db")).unwrap());

        let res = export_prompt(&db, "rust:missing::func", ExportFormat::Xml).await;
        assert!(matches!(res, Err(ExporterError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_export_prompt_success() {
        let dir = tempdir().unwrap();
        let db = Arc::new(LenaDb::open(dir.path().join("export_success.db")).unwrap());

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
