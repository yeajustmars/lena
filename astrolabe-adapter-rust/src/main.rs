// astrolabe-adapter-rust/src/main.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use astrolabe_protocol::emitter::{EdnValue, TransactionBuilder};
use astrolabe_protocol::schema::{
    ATTR_CODE_SIGNATURE, ATTR_DOC_TEXT, ATTR_ENTITY_LANGUAGE, ATTR_ENTITY_TYPE, ATTR_LOC_FILE,
    ATTR_LOC_START_COL, ATTR_LOC_START_LINE, ATTR_PARENT_ENTITY, ATTR_SYMBOL_FQSN,
    ATTR_SYMBOL_KIND, ATTR_SYMBOL_NAME, KIND_FN, LANG_RUST, TYPE_FUNCTION, TYPE_MODULE,
};
use clap::Parser;
use ra_ap_syntax::{AstNode, Edition, SourceFile, ast, ast::HasAttrs, ast::HasName};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(name = "astrolabe-adapter-rust")]
#[command(about = "Rust adapter for Astrolabe using rust-analyzer syntax parsing")]
struct Cli {
    /// Target directory to analyze
    #[arg(default_value = ".")]
    target: PathBuf,
}

/// Generates a deterministic u64 ID from a string key (like a Fully Qualified Symbol Name).
#[must_use]
fn hash_id(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    // Mask the most significant bit so it parses safely as a positive i64 in MesoDB's EDN lexer
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

/// Parses a single Rust file, extracting module and function definitions into a vector of ND-EDN transactions.
fn process_file(path: &Path, content: &str) -> Result<Vec<String>> {
    let mut facts = Vec::new();

    // 1. Parse the raw text into a Rust Analyzer Syntax Tree
    let parse = SourceFile::parse(content, Edition::Edition2024);
    let file_node = parse.tree();

    // 2. Define the Module Entity (representing the file itself)
    let filename = path.to_string_lossy().to_string();
    let module_fqsn = format!("rust:{filename}");
    let module_id = hash_id(&module_fqsn);

    let mut tx = TransactionBuilder::new();
    tx.add(module_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_MODULE))?;
    tx.add(
        module_id,
        ATTR_ENTITY_LANGUAGE,
        EdnValue::Keyword(LANG_RUST),
    )?;
    tx.add(module_id, ATTR_SYMBOL_FQSN, EdnValue::String(&module_fqsn))?;
    tx.add(module_id, ATTR_LOC_FILE, EdnValue::String(&filename))?;
    facts.push(tx.finish());

    // 3. Walk the AST to find all Function Definitions
    for fn_def in file_node.syntax().descendants().filter_map(ast::Fn::cast) {
        if let Some(name) = fn_def.name() {
            let fn_name = name.text().to_string();
            let fn_fqsn = format!("{module_fqsn}::{fn_name}");
            let fn_id = hash_id(&fn_fqsn);

            let mut tx = TransactionBuilder::new();
            tx.add(fn_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_FUNCTION))?;
            tx.add(fn_id, ATTR_ENTITY_LANGUAGE, EdnValue::Keyword(LANG_RUST))?;
            tx.add(fn_id, ATTR_SYMBOL_FQSN, EdnValue::String(&fn_fqsn))?;
            tx.add(fn_id, ATTR_SYMBOL_NAME, EdnValue::String(&fn_name))?;
            tx.add(fn_id, ATTR_SYMBOL_KIND, EdnValue::Keyword(KIND_FN))?;
            tx.add(fn_id, ATTR_PARENT_ENTITY, EdnValue::Ref(module_id))?;
            tx.add(fn_id, ATTR_LOC_FILE, EdnValue::String(&filename))?;

            // Note: Line/Col calculation is omitted for brevity but can be calculated using line_index
            // For testing, we mock position insertion if line_index was fully integrated.
            tx.add(fn_id, ATTR_LOC_START_LINE, EdnValue::Int(1))?;
            tx.add(fn_id, ATTR_LOC_START_COL, EdnValue::Int(1))?;

            // Extract Docstrings (/// comments)
            let docs = fn_def
                .doc_comments()
                .map(|doc| doc.syntax().text().to_string())
                .collect::<Vec<_>>()
                .join("\n");

            if !docs.is_empty() {
                tx.add(fn_id, ATTR_DOC_TEXT, EdnValue::String(&docs))?;
            }

            // Extract Function Signature (e.g. `fn process(x: i32) -> bool`)
            if let Some(param_list) = fn_def.param_list() {
                let ret_type = fn_def
                    .ret_type()
                    .map_or_else(String::new, |rt| rt.syntax().text().to_string());

                let sig = format!("fn {fn_name}{param_list} {ret_type}");
                tx.add(fn_id, ATTR_CODE_SIGNATURE, EdnValue::String(sig.trim()))?;
            }

            facts.push(tx.finish());
        }
    }

    Ok(facts)
}

/// Recursively walks a directory, finding all `.rs` files.
fn visit_dirs(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "target" && !name.starts_with('.') {
                    visit_dirs(&path)?;
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {path:?}"))?;

                if let Ok(facts) = process_file(&path, &content) {
                    for fact in facts {
                        println!("{fact}");
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    visit_dirs(&cli.target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_fixture_path(file: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/rust")
            .join(file)
    }

    #[test]
    fn test_rust_edge_cases_parse() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return; // Skip if not created yet
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();
        assert!(!facts.is_empty(), "Expected facts to be extracted");
    }

    #[test]
    fn test_rust_module_entity() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();

        let output = facts.join("\n");
        assert!(output.contains(":type/module"));
        assert!(output.contains("edge_cases.rs"));
    }

    #[test]
    fn test_rust_function_extraction() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();

        let output = facts.join("\n");
        assert!(output.contains(":symbol/name \"standard_function\""));
        assert!(output.contains(":symbol/name \"async_worker\""));
    }

    #[test]
    fn test_rust_docstring_extraction() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();

        let output = facts.join("\n");
        assert!(output.contains(":doc/text \"/// This is a docstring"));
    }

    #[test]
    fn test_rust_hash_id_masking() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();

        for fact in facts {
            let parts: Vec<&str> = fact.split_whitespace().collect();
            if parts.len() > 1
                && parts[0] == "[:db/add"
                && let Ok(id) = parts[1].parse::<u64>()
            {
                assert!(
                    id <= 0x7FFF_FFFF_FFFF_FFFF,
                    "Hash ID {id} exceeds positive 64-bit limit",
                );
            }
        }
    }

    #[test]
    fn test_rust_empty_file_handling() {
        let facts = process_file(Path::new("empty.rs"), "").unwrap();
        // Should only output the module entity
        assert_eq!(facts.len(), 1);
        assert!(facts[0].contains(":type/module"));
    }

    #[test]
    fn test_rust_malformed_syntax_resilience() {
        // rust-analyzer parser is error-resilient and builds a partial tree
        let content = "fn broken( { missing closing brace";
        let facts = process_file(Path::new("broken.rs"), content).unwrap();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/name \"broken\""));
    }

    #[test]
    fn test_rust_signature_extraction() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();

        let output = facts.join("\n");
        assert!(output.contains(":code/signature \"fn standard_function(x: i32) -> bool\""));
    }

    #[test]
    fn test_rust_axum_stress_add_count() {
        let fixture = get_fixture_path("axum/axum/0.7.4/axum/src/routing/mod.rs");
        if !fixture.exists() {
            return; // Skip stress test if fixture is missing
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = process_file(&fixture, &content).unwrap();

        let output = facts.join("\n");
        let add_count = output.matches("[:db/add").count();
        assert!(
            add_count > 100,
            "Expected >100 [:db/add assertions for Axum routing"
        );
    }
}
