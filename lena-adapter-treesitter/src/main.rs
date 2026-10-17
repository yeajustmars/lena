// lena-adapter-treesitter/src/main.rs

//#![deny(warnings)]
//#![warn(clippy::all, clippy::pedantic)]
// TODO: turn on pedantic and fix all warnings and issues

use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser as TsParser, Query, QueryCursor, StreamingIterator};

const DEFAULT_PYTHON_SCM: &str = include_str!("queries/python.scm");
const DEFAULT_JAVASCRIPT_SCM: &str = include_str!("queries/javascript.scm");

#[derive(Parser, Debug)]
#[command(
    name = "lena-adapter-treesitter",
    version,
    about = "Universal Tree-sitter adapter for Lena"
)]
struct Args {
    /// The target directory or file to analyze
    #[arg(index = 1, default_value = ".")]
    target: PathBuf,

    /// The name of the language (e.g., 'python', 'javascript')
    #[arg(short, long)]
    lang: String,

    /// Optional: Path to a custom Tree-sitter query file (.scm)
    #[arg(short, long)]
    query: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    /// Maps raw symbol names (e.g., "processPayment") to a list of candidate FQSNs globally
    pub global_symbols: HashMap<String, Vec<String>>,
}

pub struct ParsedFile {
    pub path: String,
    pub source: String,
    pub tree: tree_sitter::Tree,
}

#[must_use]
fn hash_id(input: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

fn visit_dirs(dir: &Path, ext: &str, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "node_modules" && name != ".git" && name != "venv" && name != "target" {
                    visit_dirs(&path, ext, files)?;
                }
            } else if path.extension().is_some_and(|e| e == ext) {
                files.push(path);
            }
        }
    } else if dir.extension().is_some_and(|e| e == ext) {
        files.push(dir.to_path_buf());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Determine the statically compiled language
    let language = match args.lang.as_str() {
        "python" => tree_sitter_python::LANGUAGE.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        _ => {
            return Err(format!(
                "Unsupported language: {}. Supported: python, javascript",
                args.lang
            )
            .into());
        }
    };

    // Determine query source
    let query_source = if let Some(custom_query_path) = args.query {
        fs::read_to_string(custom_query_path)?
    } else {
        match args.lang.as_str() {
            "python" => DEFAULT_PYTHON_SCM.to_string(),
            "javascript" => DEFAULT_JAVASCRIPT_SCM.to_string(),
            _ => return Err(
                "No default query available for this language. Please provide one with --query."
                    .into(),
            ),
        }
    };

    let ext = match args.lang.as_str() {
        "python" => "py",
        "javascript" => "js",
        _ => return Err("Unsupported language extension mapping".into()),
    };

    let mut files = Vec::new();
    visit_dirs(&args.target, ext, &mut files)?;

    let mut table = SymbolTable::default();
    let mut parsed_files = Vec::new();
    let ts_query = Query::new(&language, &query_source)?;

    // Pass 1: Global Symbol Discovery
    for file_path in files {
        let source_code = fs::read_to_string(&file_path)?;
        let mut parser = TsParser::new();
        parser.set_language(&language)?;

        if let Some(tree) = parser.parse(&source_code, None) {
            let path_str = file_path.to_string_lossy().to_string();
            discover_symbols(
                &tree,
                &source_code,
                &ts_query,
                &path_str,
                &args.lang,
                &mut table,
            )?;

            parsed_files.push(ParsedFile {
                path: path_str,
                source: source_code,
                tree,
            });
        }
    }

    // Pass 2: Fact Emission & Cross-Module Linking
    for pf in parsed_files {
        let facts = emit_facts(
            &pf.tree, &pf.source, &ts_query, &pf.path, &args.lang, &table,
        )?;
        for fact in facts {
            println!("{fact}");
        }
    }

    Ok(())
}

// TODO: use thiserror for all Box<dyn std::error::Error>
/// Pass 1 implementation: Harvest symbol definitions globally
pub fn discover_symbols(
    tree: &tree_sitter::Tree,
    source_code: &str,
    ts_query: &Query,
    file_path: &str,
    lang_name: &str,
    table: &mut SymbolTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(ts_query, tree.root_node(), source_code.as_bytes());
    let capture_names = ts_query.capture_names();

    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let capture_name = &capture_names[capture.index as usize];
            if capture_name.starts_with("symbol.") {
                let name = capture.node.utf8_text(source_code.as_bytes())?;
                let fqsn = format!("{lang_name}:{file_path}::{name}");
                table
                    .global_symbols
                    .entry(name.to_string())
                    .or_default()
                    .push(fqsn);
            }
        }
    }
    Ok(())
}

/// Pass 2 implementation: Emit AST facts and resolve calls against the SymbolTable
#[allow(clippy::similar_names)]
pub fn emit_facts(
    tree: &tree_sitter::Tree,
    source_code: &str,
    ts_query: &Query,
    file_path: &str,
    lang_name: &str,
    table: &SymbolTable,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(ts_query, tree.root_node(), source_code.as_bytes());
    let capture_names = ts_query.capture_names();

    let mut facts = Vec::new();
    let mut symbols: Vec<(usize, usize, String)> = Vec::new();
    let mut calls = Vec::new();

    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let capture_name = &capture_names[capture.index as usize];
            if capture_name.starts_with("symbol.") {
                let entity_type = capture_name.replace("symbol.", "");
                let name = capture.node.utf8_text(source_code.as_bytes())?;
                let fqsn = format!("{lang_name}:{file_path}::{name}");

                let id = hash_id(&fqsn);
                let parent_node = capture.node.parent().unwrap_or(capture.node);
                symbols.push((
                    parent_node.start_byte(),
                    parent_node.end_byte(),
                    fqsn.clone(),
                ));

                facts.push(format!("[:db/add {id} :symbol/fqsn \"{fqsn}\"]"));
                facts.push(format!("[:db/add {id} :entity/type :type/{entity_type}]"));
                facts.push(format!("[:db/add {id} :entity/language \"{lang_name}\"]"));
                facts.push(format!("[:db/add {id} :location/file \"{file_path}\"]"));
                facts.push(format!(
                    "[:db/add {id} :location/start-line {}]",
                    capture.node.start_position().row + 1
                ));
            } else if *capture_name == "call.callee" {
                let callee_name = capture.node.utf8_text(source_code.as_bytes())?;
                calls.push((capture.node.start_byte(), callee_name.to_string()));
            }
        }
    }

    facts.sort();
    facts.dedup();

    for (call_start, callee_name) in calls {
        let mut enclosing_fqsn = None;
        let mut min_size = usize::MAX;

        for (sym_start, sym_end, fqsn) in &symbols {
            if *sym_start <= call_start && call_start <= *sym_end {
                let size = sym_end - sym_start;
                if size < min_size {
                    min_size = size;
                    enclosing_fqsn = Some(fqsn.clone());
                }
            }
        }

        if let Some(caller_fqsn) = enclosing_fqsn {
            let caller_id = hash_id(&caller_fqsn);

            // Resolve using the global SymbolTable or fallback to file-local
            let callee_fqsn = if let Some(candidates) = table.global_symbols.get(&callee_name) {
                if candidates.len() == 1 {
                    candidates[0].clone()
                } else {
                    format!("{lang_name}:{file_path}::{callee_name}")
                }
            } else {
                format!("{lang_name}:{file_path}::{callee_name}")
            };

            let call_site_fqsn = format!("call:{caller_fqsn}->{callee_fqsn}");
            let call_site_id = hash_id(&call_site_fqsn);
            let callee_id = hash_id(&callee_fqsn);

            facts.push(format!(
                "[:db/add {call_site_id} :entity/type :type/call-site]"
            ));
            facts.push(format!(
                "[:db/add {call_site_id} :symbol/fqsn \"{call_site_fqsn}\"]"
            ));
            facts.push(format!("[:db/add {call_site_id} :call/caller {caller_id}]"));
            facts.push(format!("[:db/add {call_site_id} :call/callee {callee_id}]"));
        }
    }

    let mut results = Vec::new();
    if !facts.is_empty() {
        results.push(format!("[{}]", facts.join(" ")));
    }

    Ok(results)
}

/// Wrapper to preserve unit test signatures. Acts as a single-file two-pass pipeline.
pub fn extract_facts(
    language: &Language,
    source_code: &str,
    query_source: &str,
    file_path: &str,
    lang_name: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut parser = TsParser::new();
    parser.set_language(language)?;

    let tree = parser
        .parse(source_code, None)
        .ok_or("Failed to parse source code")?;
    let ts_query = Query::new(language, query_source)?;

    let mut table = SymbolTable::default();
    discover_symbols(
        &tree,
        source_code,
        &ts_query,
        file_path,
        lang_name,
        &mut table,
    )?;

    emit_facts(&tree, source_code, &ts_query, file_path, lang_name, &table)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PY_SOURCE: &str = include_str!("../tests/fixtures/python/edge_cases.py");
    const JS_SOURCE: &str = include_str!("../tests/fixtures/javascript/edge_cases.js");

    fn run_python() -> Vec<String> {
        extract_facts(
            &tree_sitter_python::LANGUAGE.into(),
            PY_SOURCE,
            DEFAULT_PYTHON_SCM,
            "edge_cases.py",
            "python",
        )
        .unwrap()
    }

    #[test]
    fn test_python_symbol_captures() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::func_a\""));
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::MyClass\""));
        assert!(output.contains(":entity/type :type/class"));
        assert!(output.contains(":entity/type :type/function"));
    }

    #[test]
    fn test_python_call_sites() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":type/call-site"));
        assert!(output.contains(":call/caller"));
        assert!(output.contains(":call/callee"));
    }

    #[test]
    fn test_javascript_grammar_loading() {
        let facts = extract_facts(
            &tree_sitter_javascript::LANGUAGE.into(),
            JS_SOURCE,
            DEFAULT_JAVASCRIPT_SCM,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases.js::js_func\""));
        assert!(
            output
                .contains("call:javascript:edge_cases.js::js_func->javascript:edge_cases.js::log")
        );
    }

    #[test]
    fn test_chained_method_calls() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(
            output.contains("call:python:edge_cases.py::chained->python:edge_cases.py::method_b")
        );
        assert!(
            output.contains("call:python:edge_cases.py::chained->python:edge_cases.py::func_a")
        );
    }

    #[test]
    fn test_1_based_line_indexing() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":location/start-line 1"));
    }

    #[test]
    fn test_bit_mask_verification() {
        let facts = run_python();
        for fact in facts {
            let parts: Vec<&str> = fact.split_whitespace().collect();
            if let Ok(id) = parts[1].parse::<u64>() {
                assert!(
                    id <= 0x7FFF_FFFF_FFFF_FFFF,
                    "ID {} exceeds MesoDB limits!",
                    id
                );
            }
        }
    }

    #[test]
    fn test_graceful_syntax_error_handling() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::syntax_err\""));
    }

    #[test]
    fn test_deterministic_edge_ids() {
        let run1 = run_python();
        let run2 = run_python();
        assert_eq!(
            run1, run2,
            "Successive runs must produce identically hashed graph edges"
        );
    }

    #[test]
    fn test_missing_optional_captures() {
        let query_no_calls = "(function_definition name: (identifier) @symbol.function)";
        let facts = extract_facts(
            &tree_sitter_python::LANGUAGE.into(),
            PY_SOURCE,
            query_no_calls,
            "edge_cases.py",
            "python",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(!output.contains(":type/call-site"));
    }

    #[test]
    fn test_multiple_captures_in_node() {
        let facts = run_python();
        let occurrences = facts
            .iter()
            .filter(|f| {
                f.contains("[:db/add")
                    && f.contains(":symbol/fqsn \"python:edge_cases.py::MyClass\"")
            })
            .count();
        assert_eq!(
            occurrences, 1,
            "Should only assert the primary entity once per node match"
        );
    }

    #[test]
    fn test_enterprise_python() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::get_user\""));
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::process_checkout\""));
        assert!(
            output.contains("call:python:edge_cases.py::get_user->python:edge_cases.py::query")
        );
        assert!(
            output.contains("call:python:edge_cases.py::get_user->python:edge_cases.py::filter_by")
        );
        assert!(
            output.contains("call:python:edge_cases.py::get_user->python:edge_cases.py::first")
        );
        assert!(output.contains(
            "call:python:edge_cases.py::process_checkout->python:edge_cases.py::get_user"
        ));
        assert!(
            output.contains(
                "call:python:edge_cases.py::process_checkout->python:edge_cases.py::create"
            )
        );
    }

    #[test]
    fn test_enterprise_js_symbols() {
        let facts = extract_facts(
            &tree_sitter_javascript::LANGUAGE.into(),
            JS_SOURCE,
            DEFAULT_JAVASCRIPT_SCM,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases.js::handle\""));
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases.js::processPayment\""));
    }

    #[test]
    fn test_enterprise_js_calls() {
        let facts = extract_facts(
            &tree_sitter_javascript::LANGUAGE.into(),
            JS_SOURCE,
            DEFAULT_JAVASCRIPT_SCM,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(
            output.contains(
                "call:javascript:edge_cases.js::handle->javascript:edge_cases.js::process"
            )
        );
        assert!(output.contains(
            "call:javascript:edge_cases.js::processPayment->javascript:edge_cases.js::create"
        ));
    }

    const FASTAPI_SOURCE: &str =
        include_str!("../tests/fixtures/python/fastapi/fastapi/0.103.1/fastapi/router.py");

    #[test]
    fn test_stress_fastapi_routing() {
        let facts = extract_facts(
            &tree_sitter_python::LANGUAGE.into(),
            FASTAPI_SOURCE,
            DEFAULT_PYTHON_SCM,
            "fastapi_routing.py",
            "python",
        )
        .expect("Failed to extract FastAPI facts");

        let output = facts.join("\n");
        assert!(output.matches("[:db/add").count() > 100);
        assert!(output.contains(":symbol/fqsn \"python:fastapi_routing.py::APIRoute\""));
        assert!(output.contains(":symbol/fqsn \"python:fastapi_routing.py::APIRouter\""));
        assert!(output.contains(":type/call-site"));
    }

    const EXPRESS_SOURCE: &str = include_str!(
        "../tests/fixtures/javascript/expressjs/express/4.18.2/lib/express_application.js"
    );

    #[test]
    fn test_stress_express_application() {
        let facts = extract_facts(
            &tree_sitter_javascript::LANGUAGE.into(),
            EXPRESS_SOURCE,
            DEFAULT_JAVASCRIPT_SCM,
            "express_application.js",
            "javascript",
        )
        .expect("Failed to extract Express facts");

        let output = facts.join("\n");
        assert!(output.matches("[:db/add").count() > 50);
        assert!(output.contains(":symbol/fqsn \"javascript:express_application.js::init\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_treesitter_cross_file_resolution() {
        let mut table = SymbolTable::default();
        let lang = &tree_sitter_python::LANGUAGE.into();
        let query = Query::new(lang, DEFAULT_PYTHON_SCM).unwrap();

        // Pass 1: Parse first file (callee)
        let mut parser1 = TsParser::new();
        parser1.set_language(lang).unwrap();
        let tree1 = parser1
            .parse("def shared_target():\n    pass", None)
            .unwrap();
        discover_symbols(
            &tree1,
            "def shared_target():\n    pass",
            &query,
            "callee.py",
            "python",
            &mut table,
        )
        .unwrap();

        // Pass 1: Parse second file (caller)
        let mut parser2 = TsParser::new();
        parser2.set_language(lang).unwrap();
        let source2 = "def main():\n    shared_target()";
        let tree2 = parser2.parse(source2, None).unwrap();
        discover_symbols(&tree2, source2, &query, "caller.py", "python", &mut table).unwrap();

        // Pass 2: Emit facts for the caller file
        let facts = emit_facts(&tree2, source2, &query, "caller.py", "python", &table).unwrap();
        let output = facts.join("\n");

        let expected_caller = hash_id("python:caller.py::main");
        let expected_callee = hash_id("python:callee.py::shared_target");

        assert!(output.contains(&format!(":call/caller {expected_caller}")));
        assert!(output.contains(&format!(":call/callee {expected_callee}")));
    }
}
