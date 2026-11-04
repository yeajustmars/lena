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

use lena_protocol::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_ENTITY_LANGUAGE, ATTR_ENTITY_TYPE, ATTR_LOC_FILE,
    ATTR_LOC_START_LINE, ATTR_PARENT_ENTITY, ATTR_SYMBOL_CONTAINER, ATTR_SYMBOL_EXTERNAL,
    ATTR_SYMBOL_FQSN, ATTR_SYMBOL_MODULE, ATTR_SYMBOL_NAME, TYPE_CALL_SITE, TYPE_MODULE,
};

const DEFAULT_PYTHON_SCM: &str = include_str!("queries/python.scm");
const DEFAULT_JAVASCRIPT_SCM: &str = include_str!("queries/javascript.scm");
const DEFAULT_TYPESCRIPT_SCM: &str = include_str!("queries/typescript.scm");
const DEFAULT_BASH_SCM: &str = include_str!("queries/bash.scm");
const DEFAULT_JAVA_SCM: &str = include_str!("queries/java.scm");
const DEFAULT_PHP_SCM: &str = include_str!("queries/php.scm");
const DEFAULT_RUBY_SCM: &str = include_str!("queries/ruby.scm");

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

    /// The name of the language (e.g., 'python', 'javascript', 'java')
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
    pub logical_module: String,
    pub source: String,
    pub tree: tree_sitter::Tree,
}

#[must_use]
fn hash_id(input: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

fn visit_dirs(
    dir: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "node_modules"
                    && name != ".git"
                    && name != "venv"
                    && name != "target"
                    && name != "vendor"
                {
                    visit_dirs(&path, extensions, files)?;
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(path);
                }
            }
        }
    } else if let Some(ext) = dir.extension().and_then(|e| e.to_str()) {
        if extensions.contains(&ext) {
            files.push(dir.to_path_buf());
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let (language, query_default, extensions) = match args.lang.as_str() {
        "python" => (
            tree_sitter_python::LANGUAGE.into(),
            DEFAULT_PYTHON_SCM,
            vec!["py"],
        ),
        "javascript" | "js" => (
            tree_sitter_javascript::LANGUAGE.into(),
            DEFAULT_JAVASCRIPT_SCM,
            vec!["js", "jsx"],
        ),
        "typescript" | "ts" => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            DEFAULT_TYPESCRIPT_SCM,
            vec!["ts", "tsx"],
        ),
        "bash" | "sh" => (
            tree_sitter_bash::LANGUAGE.into(),
            DEFAULT_BASH_SCM,
            vec!["sh", "bash"],
        ),
        "java" => (
            tree_sitter_java::LANGUAGE.into(),
            DEFAULT_JAVA_SCM,
            vec!["java"],
        ),
        "php" => (
            tree_sitter_php::LANGUAGE_PHP.into(),
            DEFAULT_PHP_SCM,
            vec!["php"],
        ),
        "ruby" | "rb" => (
            tree_sitter_ruby::LANGUAGE.into(),
            DEFAULT_RUBY_SCM,
            vec!["rb"],
        ),
        _ => return Err(format!("Unsupported language: {}", args.lang).into()),
    };

    let query_source = if let Some(custom_query_path) = args.query {
        fs::read_to_string(custom_query_path)?
    } else {
        query_default.to_string()
    };

    let lang_tag = match args.lang.as_str() {
        "js" => "javascript",
        "ts" => "typescript",
        "rb" => "ruby",
        "sh" => "bash",
        other => other,
    };

    let mut files = Vec::new();
    visit_dirs(&args.target, &extensions, &mut files)?;

    let mut table = SymbolTable::default();
    let mut parsed_files = Vec::new();
    let ts_query = Query::new(&language, &query_source)?;

    for file_path in files {
        let source_code = fs::read_to_string(&file_path)?;
        let mut parser = TsParser::new();
        parser.set_language(&language)?;

        if let Some(tree) = parser.parse(&source_code, None) {
            let path_str = file_path.to_string_lossy().to_string();

            // Generate clean logical module name (strip extensions and leading paths)
            let mut logical_module = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Some(parent) = file_path.parent() {
                if parent.file_name().is_some() {
                    logical_module = format!("{}/{}", parent.to_string_lossy(), logical_module);
                }
            }
            logical_module = logical_module
                .replace('\\', "/")
                .trim_start_matches("./")
                .to_string();

            discover_symbols(
                &tree,
                &source_code,
                &ts_query,
                &logical_module,
                lang_tag,
                &mut table,
            )?;

            parsed_files.push(ParsedFile {
                path: path_str,
                logical_module,
                source: source_code,
                tree,
            });
        }
    }

    for pf in parsed_files {
        let facts = emit_facts(
            &pf.tree,
            &pf.source,
            &ts_query,
            &pf.path,
            &pf.logical_module,
            lang_tag,
            &table,
        )?;
        for fact in facts {
            println!("{fact}");
        }
    }

    Ok(())
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub fn discover_symbols(
    tree: &tree_sitter::Tree,
    source_code: &str,
    ts_query: &Query,
    logical_module: &str,
    lang_tag: &str,
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

                // Check if this symbol is inside a container (class/struct)
                let mut container = None;
                let mut current = capture.node.parent();
                while let Some(node) = current {
                    if node.kind() == "class_definition" || node.kind() == "class_declaration" {
                        // Find the identifier child of the class node
                        for i in 0..node.child_count() {
                            if let Some(child) = node.child(i) {
                                if child.kind() == "identifier" || child.kind() == "type_identifier"
                                {
                                    container =
                                        Some(child.utf8_text(source_code.as_bytes())?.to_string());
                                    break;
                                }
                            }
                        }
                        break;
                    }
                    current = node.parent();
                }

                let fqsn = if let Some(c) = container {
                    format!("{lang_tag}:{logical_module}::{c}::{name}")
                } else {
                    format!("{lang_tag}:{logical_module}::{name}")
                };

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

#[allow(clippy::similar_names)]
pub fn emit_facts(
    tree: &tree_sitter::Tree,
    source_code: &str,
    ts_query: &Query,
    file_path: &str,
    logical_module: &str,
    lang_tag: &str,
    table: &SymbolTable,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(ts_query, tree.root_node(), source_code.as_bytes());
    let capture_names = ts_query.capture_names();

    let mut facts = Vec::new();
    let mut symbols: Vec<(usize, usize, String)> = Vec::new();
    let mut calls = Vec::new();

    // Emit File Module Fact
    let mod_fqsn = format!("{lang_tag}:{logical_module}");
    let mod_id = hash_id(&mod_fqsn);
    facts.push(format!("[[:db/add {mod_id} {ATTR_ENTITY_TYPE} {TYPE_MODULE}] [:db/add {mod_id} {ATTR_ENTITY_LANGUAGE} \":lang/{lang_tag}\"] [:db/add {mod_id} {ATTR_SYMBOL_FQSN} \"{mod_fqsn}\"] [:db/add {mod_id} {ATTR_SYMBOL_MODULE} \"{logical_module}\"] [:db/add {mod_id} {ATTR_SYMBOL_NAME} \"{logical_module}\"] [:db/add {mod_id} {ATTR_LOC_FILE} \"{file_path}\"]]"));

    let mut emitted_externals = std::collections::HashSet::new();

    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let capture_name = &capture_names[capture.index as usize];

            if capture_name.starts_with("symbol.") {
                let entity_type = capture_name.replace("symbol.", "");
                let name = capture.node.utf8_text(source_code.as_bytes())?;

                let mut container = None;
                let mut current = capture.node.parent();
                while let Some(node) = current {
                    if node.kind() == "class_definition" || node.kind() == "class_declaration" {
                        for i in 0..node.child_count() {
                            if let Some(child) = node.child(i) {
                                if child.kind() == "identifier" || child.kind() == "type_identifier"
                                {
                                    container =
                                        Some(child.utf8_text(source_code.as_bytes())?.to_string());
                                    break;
                                }
                            }
                        }
                        break;
                    }
                    current = node.parent();
                }

                let fqsn = if let Some(ref c) = container {
                    format!("{lang_tag}:{logical_module}::{c}::{name}")
                } else {
                    format!("{lang_tag}:{logical_module}::{name}")
                };

                let id = hash_id(&fqsn);
                let parent_node = capture.node.parent().unwrap_or(capture.node);
                symbols.push((
                    parent_node.start_byte(),
                    parent_node.end_byte(),
                    fqsn.clone(),
                ));

                let mut fact = format!(
                    "[:db/add {id} {ATTR_SYMBOL_FQSN} \"{fqsn}\"] [:db/add {id} {ATTR_ENTITY_TYPE} :type/{entity_type}] [:db/add {id} {ATTR_ENTITY_LANGUAGE} \":lang/{lang_tag}\"] [:db/add {id} {ATTR_SYMBOL_NAME} \"{name}\"] [:db/add {id} {ATTR_SYMBOL_MODULE} \"{logical_module}\"] [:db/add {id} {ATTR_PARENT_ENTITY} {mod_id}] [:db/add {id} {ATTR_LOC_FILE} \"{file_path}\"] [:db/add {id} {ATTR_LOC_START_LINE} {}]",
                    capture.node.start_position().row + 1
                );

                if let Some(c) = container {
                    fact += &format!(" [:db/add {id} {ATTR_SYMBOL_CONTAINER} \"{c}\"]");
                }
                facts.push(format!("[{fact}]"));
            } else if *capture_name == "call.callee" {
                let callee_name = capture.node.utf8_text(source_code.as_bytes())?;
                calls.push((capture.node.start_byte(), callee_name.to_string()));
            }
        }
    }

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

            let (callee_fqsn, is_external) =
                if let Some(candidates) = table.global_symbols.get(&callee_name) {
                    if candidates.len() == 1 {
                        (candidates[0].clone(), false)
                    } else {
                        (format!("{lang_tag}:{logical_module}::{callee_name}"), false)
                    }
                } else {
                    // Not found in this workspace, assume external
                    (format!("{lang_tag}:external::{callee_name}"), true)
                };

            let call_site_fqsn = format!("call:{caller_fqsn}->{callee_fqsn}");
            let call_site_id = hash_id(&call_site_fqsn);
            let callee_id = hash_id(&callee_fqsn);

            let call_fact = format!(
                "[[:db/add {call_site_id} {ATTR_ENTITY_TYPE} {TYPE_CALL_SITE}] [:db/add {call_site_id} {ATTR_SYMBOL_FQSN} \"{call_site_fqsn}\"] [:db/add {call_site_id} {ATTR_CALL_CALLER} {caller_id}] [:db/add {call_site_id} {ATTR_CALL_CALLEE} {callee_id}] [:db/add {call_site_id} {ATTR_LOC_FILE} \"{file_path}\"]]"
            );
            facts.push(call_fact);

            if is_external && emitted_externals.insert(callee_id) {
                let stub_fact = format!(
                    "[[:db/add {callee_id} {ATTR_SYMBOL_FQSN} \"{callee_fqsn}\"] [:db/add {callee_id} {ATTR_SYMBOL_NAME} \"{}\"] [:db/add {callee_id} {ATTR_SYMBOL_EXTERNAL} \"true\"]]",
                    escape_string(&callee_name)
                );
                facts.push(stub_fact);
            }
        }
    }

    facts.sort();
    facts.dedup();
    Ok(facts)
}

pub fn extract_facts(
    language: &Language,
    source_code: &str,
    query_source: &str,
    file_path: &str,
    logical_module: &str,
    lang_tag: &str,
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
        logical_module,
        lang_tag,
        &mut table,
    )?;
    emit_facts(
        &tree,
        source_code,
        &ts_query,
        file_path,
        logical_module,
        lang_tag,
        &table,
    )
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
            "src/edge_cases.py",
            "edge_cases",
            "python",
        )
        .unwrap()
    }

    #[test]
    fn test_python_symbol_captures() {
        let facts = run_python();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"python:edge_cases::func_a\""));
        assert!(output.contains(":symbol/module \"edge_cases\""));
        assert!(output.contains(":symbol/container \"MyClass\""));
    }

    #[test]
    fn test_external_stub_emission() {
        let facts = run_python();
        let output = facts.join("\n");
        // func_a is local, but print (if added) or unseen calls become external stubs
        assert!(output.contains(":symbol/external \"true\""));
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
            "src/edge_cases.js",
            "edge_cases",
            "javascript",
        )
        .unwrap();
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases::js_func\""));
    }
}
