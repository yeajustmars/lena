# Module `lena-adapter-rust/src/main.rs`

**Source**: `lena-adapter-rust/src/main.rs`

## `collect_files`

```rust
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()>
```

## `discover_symbols`

```rust
fn discover_symbols(file: &ParsedFile, table: &mut SymbolTable)
```

/// Pass 1 implementation: Harvest symbol definitions globally

## `emit_call_site`

```rust
fn emit_call_site(
    caller_fqsn: &str,
    caller_id: u64,
    callee_fqsn: &str,
    filename: &str,
) -> Result<String>
```

## `emit_facts`

```rust
fn emit_facts(file: &ParsedFile, table: &SymbolTable) -> Result<Vec<String>>
```

/// Pass 2 implementation: Emit AST facts and resolve calls against the SymbolTable

## `emit_function_facts`

```rust
fn emit_function_facts(
    fn_def: &ast::Fn,
    filename: &str,
    module_fqsn: &str,
    module_id: u64,
    local_imports: Option<&HashMap<String, String>>,
) -> Result<Vec<String>>
```

## `get_fixture_path`

```rust
fn get_fixture_path(file: &str) -> PathBuf
```

## `hash_id`

```rust
fn hash_id(key: &str) -> u64
```

## `main`

```rust
fn main() -> Result<()>
```

## `new`

```rust
fn new() -> Self
```

## `run_two_pass_on_fixture`

```rust
fn run_two_pass_on_fixture(path: &Path, content: &str) -> Vec<String>
```

/// Simulates the two-pass architecture for a single file to support unit tests.

## `test_rust_axum_stress_add_count`

```rust
fn test_rust_axum_stress_add_count()
```

## `test_rust_call_site_extraction`

```rust
fn test_rust_call_site_extraction()
```

## `test_rust_cross_file_resolution`

```rust
fn test_rust_cross_file_resolution()
```

## `test_rust_edge_cases_parse`

```rust
fn test_rust_edge_cases_parse()
```

## `test_rust_function_extraction`

```rust
fn test_rust_function_extraction()
```

## `test_rust_hash_id_masking`

```rust
fn test_rust_hash_id_masking()
```

## `test_rust_module_entity`

```rust
fn test_rust_module_entity()
```

## `visit_dirs`

```rust
fn visit_dirs(dir: &Path) -> Result<()>
```

## `Unnamed`

