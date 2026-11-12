// lena-adapter-rust/src/main.rs

#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]

use anyhow::Result;
use clap::Parser;
use ra_ap_syntax::{AstNode, Edition, SourceFile, ast, ast::HasAttrs, ast::HasName};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use lena_protocol::emitter::{EdnValue, TransactionBuilder};
use lena_protocol::schema::{
    ATTR_CALL_CALLEE, ATTR_CALL_CALLER, ATTR_CODE_SIGNATURE, ATTR_DOC_TEXT, ATTR_ENTITY_LANGUAGE,
    ATTR_ENTITY_TYPE, ATTR_LOC_FILE, ATTR_LOC_START_COL, ATTR_LOC_START_LINE, ATTR_PARENT_ENTITY,
    ATTR_SYMBOL_CONTAINER, ATTR_SYMBOL_EXTERNAL, ATTR_SYMBOL_FQSN, ATTR_SYMBOL_KIND,
    ATTR_SYMBOL_MODULE, ATTR_SYMBOL_NAME, KIND_FN, KIND_METHOD, LANG_RUST, TYPE_CALL_SITE,
    TYPE_FUNCTION, TYPE_MODULE,
};

#[derive(Parser)]
#[command(name = "lena-adapter-rust")]
#[command(about = "Rust adapter for Lena using rust-analyzer syntax parsing")]
struct Cli {
    /// Target directory to analyze
    #[arg(default_value = ".")]
    target: PathBuf,
}

#[derive(Debug, Default)]
struct SymbolTable {
    pub file_to_module: HashMap<String, String>,
    pub symbols: HashMap<String, u64>,
    pub use_maps: HashMap<String, HashMap<String, String>>,
    pub local_fqsns: HashSet<String>,
}

impl SymbolTable {
    #[must_use]
    fn new() -> Self {
        Self::default()
    }
}

struct ParsedFile {
    path: String,
    logical_module: String,
    tree: ra_ap_syntax::SourceFile,
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
        r#"@DIAG:{{"level":"{level}","file":{f},"line":{line},"col":{col},"symbol":{sym},"category":"{category}","message":{msg}}}"#
    );
}

fn emit_prog_init(total: usize) {
    eprintln!(r#"@PROG:{{"status":"init","total":{total}}}"#);
}

fn emit_prog_parsing(file: &str) {
    let f = serde_json::to_string(file).unwrap_or_else(|_| "\"\"".to_string());
    eprintln!(r#"@PROG:{{"status":"parsing","file":{f}}}"#);
}

#[must_use]
fn hash_id(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF
}

fn discover_symbols(file: &ParsedFile, table: &mut SymbolTable) {
    let mut local_imports = HashMap::new();

    for use_item in file.tree.syntax().descendants().filter_map(ast::Use::cast) {
        if let Some(use_tree) = use_item.use_tree()
            && let Some(path) = use_tree.path()
        {
            let path_str = path.syntax().text().to_string().replace(' ', "");
            let local_name = if let Some(rename) = use_tree.rename() {
                rename.name().map(|n| n.text().to_string())
            } else {
                path.segment()
                    .and_then(|s| s.name_ref())
                    .map(|n| n.text().to_string())
            };

            if let Some(name) = local_name {
                local_imports.insert(name, format!("rust:{path_str}"));
            }
        }
    }
    table.use_maps.insert(file.path.clone(), local_imports);

    for fn_def in file.tree.syntax().descendants().filter_map(ast::Fn::cast) {
        if let Some(name) = fn_def.name() {
            let fn_name = name.text().to_string();

            let container = get_container_name(&fn_def);
            let fn_fqsn = if let Some(c) = &container {
                format!("rust:{}::{}::{}", file.logical_module, c, fn_name)
            } else {
                format!("rust:{}::{}", file.logical_module, fn_name)
            };

            let fn_id = hash_id(&fn_fqsn);
            table.symbols.insert(fn_fqsn.clone(), fn_id);
            table.local_fqsns.insert(fn_fqsn);
        }
    }
}

fn get_container_name(fn_def: &ast::Fn) -> Option<String> {
    let mut current = fn_def.syntax().parent();
    while let Some(node) = current {
        if let Some(impl_block) = ast::Impl::cast(node.clone())
            && let Some(ty) = impl_block.self_ty()
        {
            return Some(ty.syntax().text().to_string());
        }
        current = node.parent();
    }
    None
}

fn emit_module_fact(file: &ParsedFile, facts: &mut Vec<String>) -> Result<u64> {
    let filename = &file.path;
    let logical_module = &file.logical_module;
    let module_fqsn = format!("rust:{logical_module}");
    let module_id = hash_id(&module_fqsn);

    let mut tx = TransactionBuilder::new();
    tx.add(module_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_MODULE))?;
    tx.add(
        module_id,
        ATTR_ENTITY_LANGUAGE,
        EdnValue::Keyword(LANG_RUST),
    )?;
    tx.add(module_id, ATTR_SYMBOL_FQSN, EdnValue::String(&module_fqsn))?;
    tx.add(
        module_id,
        ATTR_SYMBOL_MODULE,
        EdnValue::String(logical_module),
    )?;

    tx.add(module_id, ATTR_LOC_FILE, EdnValue::String(filename))?;
    let mod_name = logical_module.split("::").last().unwrap_or(logical_module);
    tx.add(module_id, ATTR_SYMBOL_NAME, EdnValue::String(mod_name))?;

    facts.push(tx.finish());
    Ok(module_id)
}

fn emit_function_facts(
    fn_def: &ast::Fn,
    file: &ParsedFile,
    module_id: u64,
    table: &SymbolTable,
    facts: &mut Vec<String>,
    emitted_externals: &mut HashSet<String>,
) -> Result<()> {
    let Some(name) = fn_def.name() else {
        return Ok(());
    };

    let fn_name = name.text().to_string();
    let container = get_container_name(fn_def);
    let logical_module = &file.logical_module;
    let filename = &file.path;

    let fn_fqsn = if let Some(ref c) = container {
        format!("rust:{logical_module}::{c}::{fn_name}")
    } else {
        format!("rust:{logical_module}::{fn_name}")
    };
    let fn_id = hash_id(&fn_fqsn);

    let mut tx = TransactionBuilder::new();
    tx.add(fn_id, ATTR_ENTITY_TYPE, EdnValue::Keyword(TYPE_FUNCTION))?;
    tx.add(fn_id, ATTR_ENTITY_LANGUAGE, EdnValue::Keyword(LANG_RUST))?;
    tx.add(fn_id, ATTR_SYMBOL_FQSN, EdnValue::String(&fn_fqsn))?;
    tx.add(fn_id, ATTR_SYMBOL_NAME, EdnValue::String(&fn_name))?;
    tx.add(fn_id, ATTR_SYMBOL_MODULE, EdnValue::String(logical_module))?;

    if let Some(ref c) = container {
        tx.add(fn_id, ATTR_SYMBOL_CONTAINER, EdnValue::String(c))?;
        tx.add(fn_id, ATTR_SYMBOL_KIND, EdnValue::Keyword(KIND_METHOD))?;
    } else {
        tx.add(fn_id, ATTR_SYMBOL_KIND, EdnValue::Keyword(KIND_FN))?;
    }

    tx.add(fn_id, ATTR_PARENT_ENTITY, EdnValue::Ref(module_id))?;
    tx.add(fn_id, ATTR_LOC_FILE, EdnValue::String(filename))?;
    tx.add(fn_id, ATTR_LOC_START_LINE, EdnValue::Int(1))?;
    tx.add(fn_id, ATTR_LOC_START_COL, EdnValue::Int(1))?;

    let docs = fn_def
        .doc_comments()
        .map(|doc| {
            doc.syntax()
                .text()
                .to_string()
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim_start_matches(' ')
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    if !docs.is_empty() {
        tx.add(fn_id, ATTR_DOC_TEXT, EdnValue::String(&docs))?;
    }

    if let Some(param_list) = fn_def.param_list() {
        let ret_type = fn_def
            .ret_type()
            .map_or_else(String::new, |rt| rt.syntax().text().to_string());
        let sig = format!("fn {fn_name}{param_list} {ret_type}");
        tx.add(fn_id, ATTR_CODE_SIGNATURE, EdnValue::String(sig.trim()))?;
    }

    facts.push(tx.finish());

    // Process the function body for call sites
    if let Some(body) = fn_def.body() {
        emit_call_sites(
            &body,
            &fn_fqsn,
            fn_id,
            file,
            table,
            facts,
            emitted_externals,
        )?;
    }

    Ok(())
}

fn emit_call_site(
    caller_source_fqsn: &str,
    caller_source_id: u64,
    callee_target_fqsn: &str,
    filename: &str,
) -> Result<String> {
    let call_site_fqsn = format!("call:{caller_source_fqsn}->{callee_target_fqsn}");
    let call_site_id = hash_id(&call_site_fqsn);
    let callee_target_id = hash_id(callee_target_fqsn);

    let mut ctx = TransactionBuilder::new();
    ctx.add(
        call_site_id,
        ATTR_ENTITY_TYPE,
        EdnValue::Keyword(TYPE_CALL_SITE),
    )?;
    ctx.add(
        call_site_id,
        ATTR_CALL_CALLER,
        EdnValue::Ref(caller_source_id),
    )?;
    ctx.add(
        call_site_id,
        ATTR_CALL_CALLEE,
        EdnValue::Ref(callee_target_id),
    )?;
    ctx.add(call_site_id, ATTR_LOC_FILE, EdnValue::String(filename))?;
    ctx.add(call_site_id, ATTR_LOC_START_LINE, EdnValue::Int(1))?;
    ctx.add(call_site_id, ATTR_LOC_START_COL, EdnValue::Int(1))?;
    Ok(ctx.finish())
}

fn emit_call_sites(
    body: &ast::BlockExpr,
    fn_fqsn: &str,
    fn_id: u64,
    file: &ParsedFile,
    table: &SymbolTable,
    facts: &mut Vec<String>,
    emitted_externals: &mut HashSet<String>,
) -> Result<()> {
    let local_imports = table.use_maps.get(&file.path);
    let logical_module = &file.logical_module;
    let filename = &file.path;

    // Standard Function Calls
    for call_expr in body.syntax().descendants().filter_map(ast::CallExpr::cast) {
        if let Some(callee_expr) = call_expr.expr() {
            let raw_callee = callee_expr.syntax().text().to_string();

            let (callee_name, callee_target_fqsn) = if let Some(imported_fqsn) =
                local_imports.and_then(|i| i.get(&raw_callee).cloned())
            {
                let short_name = raw_callee
                    .split("::")
                    .last()
                    .unwrap_or(&raw_callee)
                    .to_string();
                (short_name, imported_fqsn)
            } else if raw_callee.contains("::") {
                let short_name = raw_callee
                    .split("::")
                    .last()
                    .unwrap_or(&raw_callee)
                    .to_string();
                (short_name, format!("rust:{raw_callee}"))
            } else {
                (
                    raw_callee.clone(),
                    format!("rust:{logical_module}::{raw_callee}"),
                )
            };

            facts.push(emit_call_site(
                fn_fqsn,
                fn_id,
                &callee_target_fqsn,
                filename,
            )?);

            if !table.local_fqsns.contains(&callee_target_fqsn)
                && emitted_externals.insert(callee_target_fqsn.clone())
            {
                facts.push(emit_external_stub(&callee_target_fqsn, &callee_name)?);
            }
        }
    }

    // Method calls (e.g. `obj.method()`)
    for method_call in body
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        if let Some(name_ref) = method_call.name_ref() {
            let callee_name = name_ref.text().to_string();
            let callee_target_fqsn = format!("rust:{logical_module}::[UnknownType]::{callee_name}");

            facts.push(emit_call_site(
                fn_fqsn,
                fn_id,
                &callee_target_fqsn,
                filename,
            )?);
            if !table.local_fqsns.contains(&callee_target_fqsn)
                && emitted_externals.insert(callee_target_fqsn.clone())
            {
                facts.push(emit_external_stub(&callee_target_fqsn, &callee_name)?);
            }
        }
    }

    Ok(())
}

fn emit_facts(file: &ParsedFile, table: &SymbolTable) -> Result<Vec<String>> {
    let mut facts = Vec::new();
    let mut emitted_externals = HashSet::new();

    let module_id = emit_module_fact(file, &mut facts)?;

    for fn_def in file.tree.syntax().descendants().filter_map(ast::Fn::cast) {
        emit_function_facts(
            &fn_def,
            file,
            module_id,
            table,
            &mut facts,
            &mut emitted_externals,
        )?;
    }

    Ok(facts)
}

fn emit_external_stub(callee_target_fqsn: &str, callee_name: &str) -> Result<String> {
    let callee_target_id = hash_id(callee_target_fqsn);
    let mut tx = TransactionBuilder::new();
    tx.add(
        callee_target_id,
        ATTR_SYMBOL_FQSN,
        EdnValue::String(callee_target_fqsn),
    )?;
    tx.add(
        callee_target_id,
        ATTR_SYMBOL_NAME,
        EdnValue::String(callee_name),
    )?;
    tx.add(
        callee_target_id,
        ATTR_SYMBOL_MODULE,
        EdnValue::String("external"),
    )?;
    tx.add(callee_target_id, ATTR_SYMBOL_EXTERNAL, EdnValue::Bool(true))?;
    Ok(tx.finish())
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "target" && !name.starts_with('.') {
                    collect_files(&path, files)?;
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn find_package_name(file_path: &Path, cache: &mut HashMap<PathBuf, String>) -> String {
    let mut current_dir = file_path.parent();
    while let Some(dir) = current_dir {
        if let Some(cached) = cache.get(dir) {
            return cached.clone();
        }

        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists()
            && let Ok(content) = fs::read_to_string(&cargo_toml)
        {
            let mut in_package = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed == "[package]" {
                    in_package = true;
                } else if trimmed.starts_with('[') {
                    in_package = false;
                }
                if in_package
                    && trimmed.starts_with("name")
                    && let Some(val) = trimmed.split('=').nth(1)
                {
                    let pkg_name = val
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .to_string();
                    cache.insert(dir.to_path_buf(), pkg_name.clone());
                    return pkg_name;
                }
            }
        }
        current_dir = dir.parent();
    }
    "crate".to_string()
}

fn visit_dirs(dir: &Path) -> Result<()> {
    let mut files_to_process = Vec::new();
    collect_files(dir, &mut files_to_process)?;

    let mut symbol_table = SymbolTable::new();
    let mut parsed_files = Vec::new();
    let mut pkg_cache = HashMap::new();

    emit_prog_init(files_to_process.len());

    for path in &files_to_process {
        let path_str = path.to_string_lossy().to_string();
        emit_prog_parsing(&path_str);
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                emit_diag("error", &path_str, 1, 1, "", "IoError", &e.to_string());
                continue;
            }
        };
        let parse = SourceFile::parse(&content, Edition::Edition2024);

        let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let package_name = find_package_name(path, &mut pkg_cache);
        let logical_module = format!("{package_name}::{file_stem}");

        symbol_table
            .file_to_module
            .insert(path_str.clone(), logical_module.clone());

        let parsed_file = ParsedFile {
            path: path_str,
            logical_module,
            tree: parse.tree(),
        };

        discover_symbols(&parsed_file, &mut symbol_table);
        parsed_files.push(parsed_file);
    }

    for parsed_file in &parsed_files {
        match emit_facts(parsed_file, &symbol_table) {
            Ok(facts) => {
                for fact in facts {
                    println!("{fact}");
                }
            }
            Err(e) => {
                emit_diag(
                    "error",
                    &parsed_file.path,
                    1,
                    1,
                    "",
                    "EmissionError",
                    &e.to_string(),
                );
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

    fn run_two_pass_on_fixture(path: &Path, content: &str) -> Vec<String> {
        let mut symbol_table = SymbolTable::new();
        let path_str = path.to_string_lossy().to_string();
        let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let logical_module = format!("crate::{file_stem}");

        let parse = SourceFile::parse(content, Edition::Edition2024);
        let parsed_file = ParsedFile {
            path: path_str.clone(),
            logical_module: logical_module.clone(),
            tree: parse.tree(),
        };

        symbol_table.file_to_module.insert(path_str, logical_module);

        discover_symbols(&parsed_file, &mut symbol_table);
        emit_facts(&parsed_file, &symbol_table).unwrap_or_default()
    }

    #[test]
    fn test_rust_module_entity() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = run_two_pass_on_fixture(&fixture, &content);

        let output = facts.join("\n");
        assert!(output.contains(":type/module"));
        assert!(output.contains(":symbol/module \"crate::edge_cases\""));
    }

    #[test]
    fn test_rust_container_method_extraction() {
        let fixture = get_fixture_path("edge_cases.rs");
        if !fixture.exists() {
            return;
        }
        let content = fs::read_to_string(&fixture).unwrap();
        let facts = run_two_pass_on_fixture(&fixture, &content);
        let output = facts.join("\n");

        // Method inside `impl MyStruct` in edge_cases.rs
        assert!(output.contains(":symbol/container \"MyStruct\""));
        assert!(output.contains(":symbol/kind :kind/method"));
        assert!(output.contains(":symbol/name \"method_call\""));
    }

    #[test]
    fn test_rust_external_stub_emission() {
        let mut symbol_table = SymbolTable::new();
        let file = ParsedFile {
            path: "src/test.rs".to_string(),
            logical_module: "crate::test".to_string(),
            tree: SourceFile::parse(
                "fn caller() { std::process::exit(0); }",
                Edition::Edition2024,
            )
            .tree(),
        };

        discover_symbols(&file, &mut symbol_table);
        let facts = emit_facts(&file, &symbol_table).unwrap();
        let output = facts.join("\n");

        assert!(output.contains(":symbol/external true"));
        assert!(output.contains(":symbol/fqsn \"rust:std::process::exit\""));
    }
}
