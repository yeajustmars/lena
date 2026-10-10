// astrolabe-adapter-treesitter/src/main.rs

use clap::Parser;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tree_sitter::{Language, Parser as TsParser, Query, QueryCursor, StreamingIterator};

const DEFAULT_PYTHON_SCM: &str = include_str!("../tests/fixtures/python.scm");
const DEFAULT_JAVASCRIPT_SCM: &str = include_str!("../tests/fixtures/javascript.scm");

#[derive(Parser, Debug)]
#[command(
    name = "astrolabe-adapter-treesitter",
    version,
    about = "Universal Tree-sitter adapter for Astrolabe"
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

    /// The path to the compiled dynamic Tree-sitter grammar (.so, .dylib, or .dll)
    #[arg(short, long)]
    grammar: PathBuf,
}

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

    // Load the dynamic grammar
    let lib = unsafe { libloading::Library::new(&args.grammar)? };
    let symbol_name = format!("tree_sitter_{}", args.lang);
    let language = unsafe {
        let func: libloading::Symbol<unsafe extern "C" fn() -> Language> =
            lib.get(symbol_name.as_bytes())?;
        func()
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

    // Pass 1 & 2: Process file by file
    for file_path in files {
        let source_code = fs::read_to_string(&file_path)?;
        let facts = extract_facts(
            &language,
            &source_code,
            &query_source,
            &file_path.to_string_lossy(),
            &args.lang,
        )?;

        for fact in facts {
            println!("{fact}");
        }
    }

    std::mem::forget(lib);
    Ok(())
}

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
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&ts_query, tree.root_node(), source_code.as_bytes());
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
                let fqsn = format!("{}:{}::{}", lang_name, file_path, name);

                let id = hash_id(&fqsn);
                let parent_node = capture.node.parent().unwrap_or(capture.node);
                symbols.push((
                    parent_node.start_byte(),
                    parent_node.end_byte(),
                    fqsn.clone(),
                ));

                facts.push(format!("[:db/add {} :symbol/fqsn \"{}\"]", id, fqsn));
                facts.push(format!(
                    "[:db/add {} :entity/type :type/{}]",
                    id, entity_type
                ));
                facts.push(format!(
                    "[:db/add {} :entity/language \"{}\"]",
                    id, lang_name
                ));
                facts.push(format!("[:db/add {} :location/file \"{}\"]", id, file_path));
                facts.push(format!(
                    "[:db/add {} :location/start-line {}]",
                    id,
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
            let call_site_fqsn = format!("call:{}->{}", caller_fqsn, callee_name);
            let call_site_id = hash_id(&call_site_fqsn);

            // File-local fallback for now
            let callee_id = hash_id(&format!("{}:{}::{}", lang_name, file_path, callee_name));

            facts.push(format!(
                "[:db/add {} :entity/type :type/call-site]",
                call_site_id
            ));
            facts.push(format!(
                "[:db/add {} :symbol/fqsn \"{}\"]",
                call_site_id, call_site_fqsn
            ));
            facts.push(format!(
                "[:db/add {} :call/caller {}]",
                call_site_id, caller_id
            ));
            facts.push(format!(
                "[:db/add {} :call/callee {}]",
                call_site_id, callee_id
            ));
        }
    }

    let mut results = Vec::new();
    if !facts.is_empty() {
        results.push(format!("[{}]", facts.join(" ")));
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PY_SOURCE: &str = include_str!("../tests/fixtures/python/edge_cases.py");
    const PY_QUERY: &str = include_str!("../tests/fixtures/python.scm");

    const JS_SOURCE: &str = include_str!("../tests/fixtures/javascript/edge_cases.js");
    const JS_QUERY: &str = include_str!("../tests/fixtures/javascript.scm");

    fn run_python() -> Vec<String> {
        extract_facts(
            &tree_sitter_python::LANGUAGE.into(),
            PY_SOURCE,
            PY_QUERY,
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
            JS_QUERY,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases.js::js_func\""));
        assert!(output.contains("call:javascript:edge_cases.js::js_func->log"));
    }

    #[test]
    fn test_chained_method_calls() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains("call:python:edge_cases.py::chained->method_b"));
        assert!(output.contains("call:python:edge_cases.py::chained->func_a"));
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
        assert!(output.contains("call:python:edge_cases.py::get_user->query"));
        assert!(output.contains("call:python:edge_cases.py::get_user->filter_by"));
        assert!(output.contains("call:python:edge_cases.py::get_user->first"));
        assert!(output.contains("call:python:edge_cases.py::process_checkout->get_user"));
        assert!(output.contains("call:python:edge_cases.py::process_checkout->create"));
    }

    #[test]
    fn test_enterprise_js_symbols() {
        let facts = extract_facts(
            &tree_sitter_javascript::LANGUAGE.into(),
            JS_SOURCE,
            JS_QUERY,
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
            JS_QUERY,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(output.contains("call:javascript:edge_cases.js::handle->process"));
        assert!(output.contains("call:javascript:edge_cases.js::processPayment->create"));
    }

    const FASTAPI_SOURCE: &str =
        include_str!("../tests/fixtures/python/fastapi/fastapi/0.103.1/fastapi/router.py");

    #[test]
    fn test_stress_fastapi_routing() {
        let facts = extract_facts(
            &tree_sitter_python::LANGUAGE.into(),
            FASTAPI_SOURCE,
            PY_QUERY,
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
            JS_QUERY,
            "express_application.js",
            "javascript",
        )
        .expect("Failed to extract Express facts");

        let output = facts.join("\n");
        assert!(output.matches("[:db/add").count() > 50);
        assert!(output.contains(":symbol/fqsn \"javascript:express_application.js::init\""));
        assert!(output.contains(":type/call-site"));
    }
}
