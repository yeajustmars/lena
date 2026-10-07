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

/// Hashes a string into a 64-bit integer, masking the high bit to ensure
/// it fits safely into MesoDB's signed i64 bounds.
// TODO: Pretty sure hash_id is duplicated in other places, make sure and move to central location
fn hash_id(input: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
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
    let mut results = Vec::new();

    while let Some(m) = matches.next() {
        let mut facts = Vec::new();
        let mut fqsn = String::new();

        for capture in m.captures() {
            let capture_name = &capture_names[capture.index as usize];
            if capture_name.starts_with("symbol.") {
                let entity_type = capture_name.replace("symbol.", "");
                fqsn = format!(
                    "{}:{}::{}",
                    lang_name,
                    file_path,
                    capture.node.utf8_text(source_code.as_bytes())?
                );

                let id = hash_id(&fqsn);
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
            }
        }

        if !fqsn.is_empty() {
            let caller_id = hash_id(&fqsn);
            for capture in m.captures() {
                let capture_name = &capture_names[capture.index as usize];
                if *capture_name == "call.callee" {
                    let callee_name = capture.node.utf8_text(source_code.as_bytes())?;
                    let call_site_id = hash_id(&format!("call:{}->{}", fqsn, callee_name));
                    let callee_id =
                        hash_id(&format!("{}:{}::{}", lang_name, file_path, callee_name));

                    facts.push(format!(
                        "[:db/add {} :entity/type :type/call-site]",
                        call_site_id
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
            if !facts.is_empty() {
                results.push(format!("[{}]", facts.join(" ")));
            }
        }
    }
    Ok(results)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_facts_python() {
        let language = tree_sitter_python::LANGUAGE.into();
        let source_code = "def target_func():\n    pass\n\ndef main():\n    target_func()\n";
        let query_source = r#"
        (function_definition name:
            (identifier)
            @symbol.function)
        (function_definition name:
            (identifier)
            @symbol.function
            body: (block (expression_statement (call function: (identifier) @call.callee))))
        "#;

        let facts =
            extract_facts(language, source_code, query_source, "test.py", "python").unwrap();
        let output = facts.join("\n");

        assert!(output.contains(":symbol/fqsn \"python:test.py::target_func\""));
        assert!(output.contains(":symbol/fqsn \"python:test.py::main\""));
        assert!(output.contains(":type/call-site"));
    }
}
