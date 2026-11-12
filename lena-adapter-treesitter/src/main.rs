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

fn emit_diag(
    level: &str,
    file: &str,
    line: i64,
    col: i64,
    symbol: &str,
    category: &str,
    message: &str,
) {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".to_string());
    let sym = serde_json::to_string(symbol).unwrap_or_else(|_| "\"\"".to_string());
    let f = serde_json::to_string(file).unwrap_or_else(|_| "\"\"".to_string());
    eprintln!(
        r#"@DIAG:{{"level":"{}","file":{},"line":{},"col":{},"symbol":{},"category":"{}","message":{}}}"#,
        level, f, line, col, sym, category, msg
    );
}

fn emit_prog_init(total: usize) {
    eprintln!(r#"@PROG:{{"status":"init","total":{}}}"#, total);
}

fn emit_prog_parsing(file: &str) {
    let f = serde_json::to_string(file).unwrap_or_else(|_| "\"\"".to_string());
    eprintln!(r#"@PROG:{{"status":"parsing","file":{}}}"#, f);
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
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && extensions.contains(&ext)
            {
                files.push(path);
            }
        }
    } else if let Some(ext) = dir.extension().and_then(|e| e.to_str())
        && extensions.contains(&ext)
    {
        files.push(dir.to_path_buf());
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

    emit_prog_init(files.len());

    for file_path in files {
        let path_str = file_path.to_string_lossy().to_string();
        emit_prog_parsing(&path_str);

        let source_code = match fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                emit_diag("error", &path_str, 1, 1, "", "IoError", &e.to_string());
                continue;
            }
        };
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
            if let Some(parent) = file_path.parent()
                && parent.file_name().is_some()
            {
                logical_module = format!("{}/{}", parent.to_string_lossy(), logical_module);
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
                let name = match capture.node.utf8_text(source_code.as_bytes()) {
                    Ok(n) => n,
                    Err(e) => {
                        let pos = capture.node.start_position();
                        emit_diag(
                            "warn",
                            logical_module,
                            (pos.row + 1) as i64,
                            (pos.column + 1) as i64,
                            "",
                            "EncodingError",
                            &e.to_string(),
                        );
                        continue;
                    }
                };

                // Check if this symbol is inside a container (class/struct)
                let mut container = None;
                let mut current = capture.node.parent();
                while let Some(node) = current {
                    if node.kind() == "class_definition"
                        || node.kind() == "class_declaration"
                        || node.kind() == "class"
                    {
                        for i in 0..node.child_count() {
                            if let Some(child) = node.child(i)
                                && (child.kind() == "identifier"
                                    || child.kind() == "type_identifier"
                                    || child.kind() == "constant"
                                    || child.kind() == "name")
                            {
                                container =
                                    Some(child.utf8_text(source_code.as_bytes())?.to_string());
                                break;
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

    // Emit File Module Fact (flattened to single-line ND-EDN with atomic name)
    let mod_fqsn = format!("{lang_tag}:{logical_module}");
    let mod_id = hash_id(&mod_fqsn);
    let atomic_mod_name = logical_module.rsplit('/').next().unwrap_or(logical_module);

    facts.push(format!(
        "[[:db/add {mod_id} {ATTR_ENTITY_TYPE} {TYPE_MODULE}] [:db/add {mod_id} {ATTR_ENTITY_LANGUAGE} \":lang/{lang_tag}\"] [:db/add {mod_id} {ATTR_SYMBOL_FQSN} \"{}\"] [:db/add {mod_id} {ATTR_SYMBOL_MODULE} \"{}\"] [:db/add {mod_id} {ATTR_SYMBOL_NAME} \"{}\"] [:db/add {mod_id} {ATTR_LOC_FILE} \"{}\"]]",
        escape_string(&mod_fqsn),
        escape_string(logical_module),
        escape_string(atomic_mod_name),
        escape_string(file_path)
    ));

    let mut emitted_externals = std::collections::HashSet::new();

    while let Some(m) = matches.next() {
        for capture in m.captures() {
            let capture_name = &capture_names[capture.index as usize];

            if capture_name.starts_with("symbol.") {
                let entity_type = capture_name.replace("symbol.", "");
                let name = match capture.node.utf8_text(source_code.as_bytes()) {
                    Ok(n) => n,
                    Err(e) => {
                        let pos = capture.node.start_position();
                        emit_diag(
                            "warn",
                            file_path,
                            (pos.row + 1) as i64,
                            (pos.column + 1) as i64,
                            "",
                            "EncodingError",
                            &e.to_string(),
                        );
                        continue;
                    }
                };

                let mut container = None;
                let mut current = capture.node.parent();
                while let Some(node) = current {
                    if node.kind() == "class_definition"
                        || node.kind() == "class_declaration"
                        || node.kind() == "class"
                    {
                        for i in 0..node.child_count() {
                            if let Some(child) = node.child(i)
                                && (child.kind() == "identifier"
                                    || child.kind() == "type_identifier"
                                    || child.kind() == "constant"
                                    || child.kind() == "name")
                            {
                                container =
                                    Some(child.utf8_text(source_code.as_bytes())?.to_string());
                                break;
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
                    "[:db/add {id} {ATTR_SYMBOL_FQSN} \"{}\"] [:db/add {id} {ATTR_ENTITY_TYPE} :type/{entity_type}] [:db/add {id} {ATTR_ENTITY_LANGUAGE} \":lang/{lang_tag}\"] [:db/add {id} {ATTR_SYMBOL_NAME} \"{}\"] [:db/add {id} {ATTR_SYMBOL_MODULE} \"{}\"] [:db/add {id} {ATTR_PARENT_ENTITY} {mod_id}] [:db/add {id} {ATTR_LOC_FILE} \"{}\"] [:db/add {id} {ATTR_LOC_START_LINE} {}]",
                    escape_string(&fqsn),
                    escape_string(name),
                    escape_string(logical_module),
                    escape_string(file_path),
                    capture.node.start_position().row + 1
                );

                if let Some(c) = container {
                    fact += &format!(
                        " [:db/add {id} {ATTR_SYMBOL_CONTAINER} \"{}\"]",
                        escape_string(&c)
                    );
                }

                facts.push(format!("[{fact}]"));
            } else if *capture_name == "call.callee" {
                if let Ok(callee_name) = capture.node.utf8_text(source_code.as_bytes()) {
                    calls.push((capture.node.start_byte(), callee_name.to_string()));
                }
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
                "[[:db/add {call_site_id} {ATTR_ENTITY_TYPE} {TYPE_CALL_SITE}] [:db/add {call_site_id} {ATTR_SYMBOL_FQSN} \"{}\"] [:db/add {call_site_id} {ATTR_CALL_CALLER} {caller_id}] [:db/add {call_site_id} {ATTR_CALL_CALLEE} {callee_id}] [:db/add {call_site_id} {ATTR_LOC_FILE} \"{}\"]]",
                escape_string(&call_site_fqsn),
                escape_string(file_path)
            );
            facts.push(call_fact);

            if is_external && emitted_externals.insert(callee_id) {
                let mut atomic_callee = callee_name.as_str();
                if let Some(idx) = atomic_callee.rfind('/') {
                    atomic_callee = &atomic_callee[idx + 1..];
                }
                if let Some(idx) = atomic_callee.rfind("::") {
                    atomic_callee = &atomic_callee[idx + 2..];
                }
                if let Some(idx) = atomic_callee.rfind('.') {
                    atomic_callee = &atomic_callee[idx + 1..];
                }
                let atomic_callee =
                    atomic_callee.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if !atomic_callee.is_empty() {
                    let stub_fact = format!(
                        "[[:db/add {callee_id} {ATTR_SYMBOL_FQSN} \"{}\"] [:db/add {callee_id} {ATTR_SYMBOL_NAME} \"{}\"] [:db/add {callee_id} {ATTR_SYMBOL_EXTERNAL} true]]",
                        escape_string(&callee_fqsn),
                        escape_string(atomic_callee)
                    );
                    facts.push(stub_fact);
                }
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

    // Reusable fixture runner
    fn run_ts_fixture(
        lang: tree_sitter::Language,
        source: &str,
        query: &str,
        path: &str,
        module: &str,
        tag: &str,
    ) -> Vec<String> {
        extract_facts(&lang, source, query, path, module, tag).expect("Failed to extract facts")
    }

    #[test]
    fn test_python_symbol_captures() {
        let source = include_str!("../tests/fixtures/python/edge_cases.py");
        let facts = run_ts_fixture(
            tree_sitter_python::LANGUAGE.into(),
            source,
            DEFAULT_PYTHON_SCM,
            "src/edge_cases.py",
            "edge_cases",
            "python",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"python:edge_cases::func_a\""));
        assert!(output.contains(":symbol/container \"MyClass\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_javascript_symbol_captures() {
        let source = include_str!("../tests/fixtures/javascript/edge_cases.js");
        let facts = run_ts_fixture(
            tree_sitter_javascript::LANGUAGE.into(),
            source,
            DEFAULT_JAVASCRIPT_SCM,
            "src/edge_cases.js",
            "edge_cases",
            "javascript",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"javascript:edge_cases::js_func\""));
    }

    #[test]
    fn test_typescript_symbol_captures() {
        let source = include_str!("../tests/fixtures/typescript/edge_cases.ts");
        let facts = run_ts_fixture(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            source,
            DEFAULT_TYPESCRIPT_SCM,
            "src/edge_cases.ts",
            "edge_cases",
            "typescript",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"typescript:edge_cases::ApiClient::fetchData\""));
        assert!(output.contains(":symbol/fqsn \"typescript:edge_cases::standalone\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_java_symbol_captures() {
        let source = include_str!("../tests/fixtures/java/EdgeCases.java");
        let facts = run_ts_fixture(
            tree_sitter_java::LANGUAGE.into(),
            source,
            DEFAULT_JAVA_SCM,
            "src/EdgeCases.java",
            "EdgeCases",
            "java",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"java:EdgeCases::EdgeCases::runProcess\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_php_symbol_captures() {
        let source = include_str!("../tests/fixtures/php/edge_cases.php");
        let facts = run_ts_fixture(
            tree_sitter_php::LANGUAGE_PHP.into(),
            source,
            DEFAULT_PHP_SCM,
            "src/edge_cases.php",
            "edge_cases",
            "php",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"php:edge_cases::Processor::handle\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_ruby_symbol_captures() {
        let source = include_str!("../tests/fixtures/ruby/edge_cases.rb");
        let facts = run_ts_fixture(
            tree_sitter_ruby::LANGUAGE.into(),
            source,
            DEFAULT_RUBY_SCM,
            "src/edge_cases.rb",
            "edge_cases",
            "ruby",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"ruby:edge_cases::Worker::perform\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_bash_symbol_captures() {
        let source = include_str!("../tests/fixtures/bash/edge_cases.sh");
        let facts = run_ts_fixture(
            tree_sitter_bash::LANGUAGE.into(),
            source,
            DEFAULT_BASH_SCM,
            "src/edge_cases.sh",
            "edge_cases",
            "bash",
        );
        let output = facts.join("\n");
        assert!(output.contains(":symbol/fqsn \"bash:edge_cases::deploy_server\""));
        assert!(output.contains(":type/call-site"));
    }

    // Helper for loading file-based fixtures
    fn run_file_fixture(
        lang: tree_sitter::Language,
        rel_path: &str,
        query: &str,
        lang_tag: &str,
    ) -> Vec<String> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel_path);
        if !path.exists() {
            return Vec::new();
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let logical_module = path.file_stem().unwrap().to_string_lossy().to_string();
        extract_facts(
            &lang,
            &source,
            query,
            &path.to_string_lossy(),
            &logical_module,
            lang_tag,
        )
        .expect("Failed to extract facts")
    }

    #[test]
    fn test_typescript_nestjs_real_world() {
        let facts = run_file_fixture(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/typescript/nestjs/packages/nest-router/router-explorer.ts",
            DEFAULT_TYPESCRIPT_SCM,
            "typescript",
        );
        if facts.is_empty() {
            return;
        }
        let output = facts.join("\n");

        assert!(
            facts.len() > 100,
            "Should extract many facts from NestJS router"
        );
        assert!(
            output.contains(":symbol/fqsn \"typescript:router-explorer::RouterExplorer::explore\"")
        );
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_java_spring_real_world() {
        let facts = run_file_fixture(
            tree_sitter_java::LANGUAGE.into(),
            "tests/fixtures/java/spring-framework/spring-webmvc/src/main/java/org/springframework/web/servlet/DispatcherServlet.java",
            DEFAULT_JAVA_SCM,
            "java",
        );
        if facts.is_empty() {
            return;
        }
        let output = facts.join("\n");

        assert!(
            facts.len() > 100,
            "Should extract hundreds of facts from DispatcherServlet"
        );
        assert!(
            output
                .contains(":symbol/fqsn \"java:DispatcherServlet::DispatcherServlet::doDispatch\"")
        );
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_php_laravel_real_world() {
        let facts = run_file_fixture(
            tree_sitter_php::LANGUAGE_PHP.into(),
            "tests/fixtures/php/laravel/illuminate/routing/Route.php",
            DEFAULT_PHP_SCM,
            "php",
        );
        if facts.is_empty() {
            return;
        }
        let output = facts.join("\n");

        assert!(facts.len() > 100, "Should extract facts from Laravel Route");
        assert!(output.contains(":symbol/fqsn \"php:Route::Route::run\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_ruby_sinatra_real_world() {
        let facts = run_file_fixture(
            tree_sitter_ruby::LANGUAGE.into(),
            "tests/fixtures/ruby/sinatra/lib/sinatra/base.rb",
            DEFAULT_RUBY_SCM,
            "ruby",
        );
        if facts.is_empty() {
            return;
        }
        let output = facts.join("\n");

        assert!(
            facts.len() > 1000,
            "Should extract massive graph from Sinatra base"
        );
        assert!(output.contains(":symbol/fqsn \"ruby:base::Base::call\""));
        assert!(output.contains(":type/call-site"));
    }

    #[test]
    fn test_bash_nvm_real_world() {
        let facts = run_file_fixture(
            tree_sitter_bash::LANGUAGE.into(),
            "tests/fixtures/bash/nvm/nvm.sh",
            DEFAULT_BASH_SCM,
            "bash",
        );
        if facts.is_empty() {
            return;
        }
        let output = facts.join("\n");

        assert!(facts.len() > 200, "Should extract facts from NVM");
        assert!(output.contains(":symbol/fqsn \"bash:nvm::nvm_echo\""));
        assert!(output.contains(":type/call-site"));
    }
}
