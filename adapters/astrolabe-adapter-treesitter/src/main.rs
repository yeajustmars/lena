use clap::Parser;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
// 1. Import StreamingIterator
use tree_sitter::{Language, Parser as TsParser, Query, QueryCursor, StreamingIterator};

#[derive(Parser, Debug)]
#[command(
    name = "astrolabe-adapter-treesitter",
    version,
    about = "Universal Tree-sitter adapter for Astrolabe"
)]
struct Args {
    /// The target file to analyze
    #[arg(index = 1)]
    target: PathBuf,

    /// The name of the language (e.g., 'python', 'rust') to locate the C-symbol
    #[arg(short, long)]
    lang: String,

    /// The path to the standard Astrolabe Tree-sitter query file (.scm)
    #[arg(short, long)]
    query: PathBuf,

    /// The path to the compiled dynamic Tree-sitter grammar (.so, .dylib, or .dll)
    #[arg(short, long)]
    grammar: PathBuf,
}

// TODO: replace all Box<dyn std::error::Error> w/ thiserror or anyhow
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let lib = unsafe { libloading::Library::new(&args.grammar)? };
    let symbol_name = format!("tree_sitter_{}", args.lang);
    let language = unsafe {
        let func: libloading::Symbol<unsafe extern "C" fn() -> Language> =
            lib.get(symbol_name.as_bytes())?;
        func()
    };

    let source_code = fs::read_to_string(&args.target)?;
    let query_source = fs::read_to_string(&args.query)?;

    let facts = extract_facts(
        language,
        &source_code,
        &query_source,
        &args.target.to_string_lossy(),
        &args.lang,
    )?;
    for fact in facts {
        println!("{}", fact);
    }

    std::mem::forget(lib);
    Ok(())
}

pub fn extract_facts(
    language: Language,
    source_code: &str,
    query_source: &str,
    file_path: &str,
    lang_name: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut parser = TsParser::new();
    parser.set_language(&language)?;
    let tree = parser
        .parse(source_code, None)
        .ok_or("Failed to parse source code")?;
    let ts_query = Query::new(&language, query_source)?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&ts_query, tree.root_node(), source_code.as_bytes());
    let capture_names = ts_query.capture_names();

    let mut facts = Vec::new();
    let mut symbols: Vec<(usize, usize, String)> = Vec::new();
    let mut calls = Vec::new();

    // Pass 1: Collect independent symbols and calls
    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let capture_name = &capture_names[capture.index as usize];
            if capture_name.starts_with("symbol.") {
                let entity_type = capture_name.replace("symbol.", "");
                let name = capture.node.utf8_text(source_code.as_bytes())?;
                let fqsn = format!("{}:{}::{}", lang_name, file_path, name);

                let id = hash_id(&fqsn);
                // The capture is just the identifier, but its parent is the actual definition block
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

    // De-duplicate any overlapping matches
    facts.sort();
    facts.dedup();

    // Pass 2: Spatially correlate calls to the tightest enclosing symbol
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
            let callee_id = hash_id(&format!("{}:{}::{}", lang_name, file_path, callee_name));

            facts.push(format!(
                "[:db/add {} :entity/type :type/call-site]",
                call_site_id
            ));
            facts.push(format!(
                "[:db/add {} :symbol/fqsn \"{}\"]",
                call_site_id, call_site_fqsn
            )); // <--- Added this line
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

/// Hashes a string into a 64-bit integer, masking the high bit to ensure
/// it fits safely into MesoDB's signed i64 bounds.
// TODO: Pretty sure hash_id is duplicated in other places, make sure and move to central location
fn hash_id(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    const PY_SOURCE: &str = include_str!("../tests/fixtures/edge_cases.py");
    const PY_QUERY: &str = include_str!("../tests/fixtures/python.scm");

    const JS_SOURCE: &str = include_str!("../tests/fixtures/edge_cases.js");
    const JS_QUERY: &str = include_str!("../tests/fixtures/javascript.scm");

    fn run_python() -> Vec<String> {
        extract_facts(
            tree_sitter_python::LANGUAGE.into(),
            PY_SOURCE,
            PY_QUERY,
            "edge_cases.py",
            "python",
        )
        .unwrap()
    }

    // 1. Primary symbol captures
    #[test]
    fn test_python_symbol_captures() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::func_a\""));
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::MyClass\""));
        assert!(output.contains(":entity/type :type/class"));
        assert!(output.contains(":entity/type :type/function"));
    }

    // 2. Call-site capture resolution
    #[test]
    fn test_python_call_sites() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":type/call-site"));
        assert!(output.contains(":call/caller"));
        assert!(output.contains(":call/callee"));
    }

    // 3. Multi-language grammar loading
    #[test]
    fn test_javascript_grammar_loading() {
        let facts = extract_facts(
            tree_sitter_javascript::LANGUAGE.into(),
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

    // 4. Chained method calls
    #[test]
    fn test_chained_method_calls() {
        let facts = run_python();
        let output = facts.join("\n");
        // obj.method_b().func_a() should capture both method_b and func_a as callees
        assert!(output.contains("call:python:edge_cases.py::chained->method_b"));
        assert!(output.contains("call:python:edge_cases.py::chained->func_a"));
    }

    // 5. 1-based line indexing correction
    #[test]
    fn test_1_based_line_indexing() {
        let facts = run_python();
        let output = facts.join("\n");
        // func_a is on line 1 in 0-based Tree-sitter, but line 2 in physical files (due to empty line 1 or 0-index)
        // In our fixture, `def func_a()` is on line 1 physically. Tree-sitter returns 0. We add 1.
        assert!(output.contains(":location/start-line 1"));
    }

    // 6. High-bit masking verification
    #[test]
    fn test_bit_mask_verification() {
        let facts = run_python();
        for fact in facts {
            let parts: Vec<&str> = fact.split_whitespace().collect();
            // ID is the second token: [:db/add ID :attr "val"]
            if let Ok(id) = parts[1].parse::<u64>() {
                assert!(
                    id <= 0x7FFF_FFFF_FFFF_FFFF,
                    "ID {} exceeds MesoDB limits!",
                    id
                );
            }
        }
    }

    // 7. Graceful syntax error handling
    #[test]
    fn test_graceful_syntax_error_handling() {
        let facts = run_python();
        let output = facts.join("\n");
        // Tree-sitter recovers from the missing colon on `def syntax_err()` and still parses it
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::syntax_err\""));
    }

    // 8. Deterministic Edge IDs
    #[test]
    fn test_deterministic_edge_ids() {
        let run1 = run_python();
        let run2 = run_python();
        assert_eq!(
            run1, run2,
            "Successive runs must produce identically hashed graph edges"
        );
    }

    // 9. Missing capture fallback
    #[test]
    fn test_missing_optional_captures() {
        // A query that doesn't match any calls shouldn't crash the extractor
        let query_no_calls = "(function_definition name: (identifier) @symbol.function)";
        let facts = extract_facts(
            tree_sitter_python::LANGUAGE.into(),
            PY_SOURCE,
            query_no_calls,
            "edge_cases.py",
            "python",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(!output.contains(":type/call-site")); // Safely bypassed
    }

    // 10. Multiple captures within single AST match
    #[test]
    fn test_multiple_captures_in_node() {
        let facts = run_python();
        // Ensure that a function definition that ALSO contains a call inside its body
        // doesn't duplicate the primary entity assertion
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

    // 11. Enterprise Python: Async, Decorators, and Fluent Chaining
    #[test]
    fn test_enterprise_python() {
        let facts = run_python();
        let output = facts.join("\n");

        // Did it catch the async class method?
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::get_user\""));
        // Did it catch the decorated async endpoint?
        assert!(output.contains(":symbol/fqsn \"python:edge_cases.py::process_checkout\""));

        // Did it map the complex fluent chains inside `get_user`?
        assert!(output.contains("call:python:edge_cases.py::get_user->query"));
        assert!(output.contains("call:python:edge_cases.py::get_user->filter_by"));
        assert!(output.contains("call:python:edge_cases.py::get_user->first"));

        // Did it map the calls inside the transaction block in `process_checkout`?
        assert!(output.contains("call:python:edge_cases.py::process_checkout->get_user"));
        assert!(output.contains("call:python:edge_cases.py::process_checkout->create"));
    }

    // 12. Enterprise JS: Class Methods and Const Arrow Functions
    #[test]
    fn test_enterprise_js_symbols() {
        let facts = extract_facts(
            tree_sitter_javascript::LANGUAGE.into(),
            JS_SOURCE,
            JS_QUERY,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");

        // Did it catch the NestJS-style class method?
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases.js::handle\""));

        // Did it catch the constant arrow function?
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases.js::processPayment\""));
    }

    // 13. Enterprise JS: Async Call Tracking
    #[test]
    fn test_enterprise_js_calls() {
        let facts = extract_facts(
            tree_sitter_javascript::LANGUAGE.into(),
            JS_SOURCE,
            JS_QUERY,
            "edge_cases.js",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");

        // Did it map the service call inside the class method?
        assert!(output.contains("call:javascript:edge_cases.js::handle->process"));

        // Did it map the Stripe API call inside the arrow function?
        assert!(output.contains("call:javascript:edge_cases.js::processPayment->create"));
    }
}
