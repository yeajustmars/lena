# Module `main`

**Source**: `./lena-adapter-rust/src/main.rs`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `add`

```rust
fn add(
        &mut self,
        entity: u64,
        attribute: &str,
        value: EdnValue,
    ) -> Result<&mut Self, fmt::Error>
```

/// Adds an assertion fact (`:db/add`) to the transaction.
///
/// # Errors
/// Returns `fmt::Error` if string formatting fails.

## `async_worker`

```rust
fn async_worker(payload: T)
```

## `badge_class`

```rust
fn badge_class(&self) -> &'static str
```

## `bootstrap`

```rust
fn bootstrap() -> SchemaMap
```

/// Bootstraps the complete Universal Intermediate Representation (UIR) schema.

## `build`

```rust
fn build(self) -> Result<DocGraph>
```

## `build_indented`

```rust
fn build_indented(
    node: &str,
    edges: &[Edge],
    depth: usize,
    path_visited: &mut HashSet<String>,
    out: &mut String,
)
```

## `call`

```rust
fn call(&mut self, req: Request<B>) -> Self::Future
```

## `call_with_state`

```rust
fn call_with_state(&mut self, req: Request, state: S) -> RouteFuture<E>
```

## `clone`

```rust
fn clone(&self) -> Self
```

## `collect_files`

```rust
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()>
```

## `contains_rule_call`

```rust
fn contains_rule_call(query: &str) -> bool
```

/// Helper to detect if a query string invokes standard recursive rule predicates.

## `criterion_benchmark`

```rust
fn criterion_benchmark(c: &mut Criterion)
```

## `default`

```rust
fn default() -> Self
```

## `discover_symbols`

```rust
fn discover_symbols(file: &ParsedFile, table: &mut SymbolTable)
```

/// Pass 1 implementation: Harvest symbol definitions globally

## `emit`

```rust
fn emit(&self, out_dir: &Path) -> Result<()>
```

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

## `execute_kondo`

```rust
fn execute_kondo(target: &Path) -> anyhow::Result<Vec<String>>
```

## `export_prompt`

```rust
fn export_prompt(
    db: &Arc<LenaDb>,
    fqsn: &str,
    format: ExportFormat,
) -> Result<String, ExporterError>
```

/// Retrieves an entity by its FQSN and formats it for LLM context.

## `extract_facts`

```rust
fn extract_facts(
    language: &Language,
    source_code: &str,
    query_source: &str,
    file_path: &str,
    lang_name: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>>
```

/// Wrapper to preserve unit test signatures. Acts as a single-file two-pass pipeline.

## `extract_i64`

```rust
fn extract_i64(val: &Value, key: &str) -> i64
```

## `extract_i64`

```rust
fn extract_i64(val: &Value, key: &str) -> i64
```

/// Helper to extract an integer from either `:key` or `key` inside a JSON Value.

## `extract_string`

```rust
fn extract_string(val: &Value, key: &str) -> String
```

/// Helper to extract a string from either `:key` or `key` inside a JSON Value.

## `extract_string`

```rust
fn extract_string(val: &Value, key: &str) -> String
```

## `fallback`

```rust
fn fallback(self, handler: H) -> Self
```

## `fallback_endpoint`

```rust
fn fallback_endpoint(self, endpoint: Endpoint<S>) -> Self
```

## `fallback_service`

```rust
fn fallback_service(self, service: T) -> Self
```

/// Add a fallback [`Service`] to the router.
///
/// See [`Router::fallback`] for more details.

## `fetch_all_symbols`

```rust
fn fetch_all_symbols(&self) -> Result<Vec<RawSymbol>>
```

## `fetch_call_graph_edges`

```rust
fn fetch_call_graph_edges(
        &self,
        symbols_map: &HashMap<String, RawSymbol>,
    ) -> Result<(
        HashMap<String, Vec<NeighborRef>>,
        HashMap<String, Vec<NeighborRef>>,
    )>
```

## `finish`

```rust
fn finish(mut self) -> String
```

/// Returns the finalized ND-EDN transaction string.

## `fmt`

```rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
```

## `fmt`

```rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
```

## `fmt`

```rust
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
```

## `format_skeleton`

```rust
fn format_skeleton(root: &str, edges: &[Edge], format: SkeletonFormat) -> String
```

## `from_str`

```rust
fn from_str(kind: &str) -> Self
```

## `generate_mermaid`

```rust
fn generate_mermaid(&mut self, current_name: &str)
```

## `generate_skeleton`

```rust
fn generate_skeleton(
    db: &Arc<LenaDb>,
    root_fqsn: &str,
    max_depth: usize,
    format: SkeletonFormat,
) -> Result<String, SkeletonError>
```

/// Iteratively queries the database to build a bounded subgraph, preventing infinite loops on cycles.

## `get_fixture_path`

```rust
fn get_fixture_path(file: &str) -> PathBuf
```

## `hash_id`

```rust
fn hash_id(key: &str) -> u64
```

## `ingest_stream`

```rust
fn ingest_stream(db: &Arc<LenaDb>, reader: R) -> Result<usize, IngestError>
```

/// Ingests ND-EDN lines from an asynchronous reader (like an adapter's stdout)
/// and streams them directly into the database.
///
/// # Errors
/// Returns `IngestError` if the underlying reader fails or if a transaction panics.

## `inner`

```rust
fn inner(&self) -> &MesoDb
```

/// Access the underlying MesoDB engine directly.

## `into_inner`

```rust
fn into_inner(self) -> RouterInner<S>
```

## `into_make_service_with_connect_info`

```rust
fn into_make_service_with_connect_info(self) -> IntoMakeServiceWithConnectInfo<Self, C>
```

## `into_service`

```rust
fn into_service(self) -> RouterIntoService<B, S>
```

/// Convert the router into an owned [`Service`] with a fixed request body type, to aid type
/// inference.
///
/// This is the same as [`Router::as_service`] instead it returns an owned [`Service`]. See
/// that method for more details.

## `layer`

```rust
fn layer(self, layer: L) -> Endpoint<S>
```

## `main`

```rust
fn main() -> anyhow::Result<()>
```

## `map`

```rust
fn map(self, f: F) -> Fallback<S, E2>
```

## `map_inner`

```rust
fn map_inner(self, f: F) -> Router<S2>
```

## `merge`

```rust
fn merge(self, other: Self) -> Option<Self>
```

## `method_call`

```rust
fn method_call(&self)
```

## `nest`

```rust
fn nest(self, path: &str, router: Router<S>) -> Self
```

## `nest_service`

```rust
fn nest_service(self, path: &str, service: T) -> Self
```

/// Like [`nest`](Self::nest), but accepts an arbitrary `Service`.

## `new`

```rust
fn new(graph: &'a DocGraph, custom_css: Option<String>) -> Result<Self>
```

## `new`

```rust
fn new() -> Self
```

## `new`

```rust
fn new() -> Self
```

## `new`

```rust
fn new(db: &'a LenaDb, options: DocGenOptions) -> Self
```

/// Create a new `Router`.
///
/// Unless you add additional routes this will respond with `404 Not Found` to
/// all requests.

## `new`

```rust
fn new(graph: &'a DocGraph) -> Self
```

## `open`

```rust
fn open(path: impl AsRef<Path>) -> Result<Self, DbError>
```

/// Opens or creates an Lena database at the specified path.
/// # Errors
/// Returns `DbError::MesoDb` if the underlying database engine fails.

## `poll_ready`

```rust
fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
```

## `process_kondo_output`

```rust
fn process_kondo_output(kondo_data: KondoOutput) -> anyhow::Result<Vec<String>>
```

/// Core processing logic extracted to allow isolated unit testing of the EDN generation

## `push_fact`

```rust
fn push_fact(
        &mut self,
        op: &str,
        entity: u64,
        attribute: &str,
        value: EdnValue,
    ) -> Result<(), fmt::Error>
```

## `query_json`

```rust
fn query_json(&self, query_str: &str) -> Result<String, DbError>
```

/// Evaluates a raw Datalog query string directly without rule transformations.
/// # Errors
/// Returns `DbError::MesoDb` if the underlying database engine fails.

## `query_with_custom_rules`

```rust
fn query_with_custom_rules(
        &self,
        query_str: &str,
        rules_str: &str,
    ) -> Result<String, DbError>
```

/// Evaluates a Datalog query with custom recursive rules provided as an EDN string.

## `query_with_rules`

```rust
fn query_with_rules(&self, query_str: &str) -> Result<String, DbError>
```

/// Evaluates a Datalog query, automatically injecting `STANDARD_RULES` if rule
/// predicates or rule bindings (`%`) are detected.
/// # Errors
/// Returns `DbError::MesoDb` if the underlying database engine fails.

## `render_landing_page`

```rust
fn render_landing_page(&self, out_dir: &Path) -> Result<()>
```

## `render_lena_book`

```rust
fn render_lena_book(&self, out_dir: &Path) -> Result<()>
```

## `render_module_page`

```rust
fn render_module_page(
        &self,
        module: &crate::docgen::ir::DocModule,
        _lang: &str,
        _pkg: &str,
    ) -> Result<String>
```

## `render_single_page`

```rust
fn render_single_page(&self, out_dir: &Path) -> Result<()>
```

## `resolve_binary`

```rust
fn resolve_binary(adapter_name: &str) -> String
```

## `retract`

```rust
fn retract(
        &mut self,
        entity: u64,
        attribute: &str,
        value: EdnValue,
    ) -> Result<&mut Self, fmt::Error>
```

/// Adds a retraction fact (`:db/retract`) to the transaction.
///
/// # Errors
/// Returns `fmt::Error` if string formatting fails.

## `route`

```rust
fn route(self, path: &str, method_router: MethodRouter<S>) -> Self
```

## `route_layer`

```rust
fn route_layer(self, layer: L) -> Self
```

## `route_service`

```rust
fn route_service(self, path: &str, service: T) -> Self
```

## `run`

```rust
fn run(db: &Arc<LenaDb>, query: &str) -> Result<()>
```

## `run`

```rust
fn run(db: &Arc<LenaDb>, fqsn: &str, depth: usize, format: &str) -> Result<()>
```

## `run`

```rust
fn run(
    target: PathBuf,
    grammars_dir: &Path,
    concurrency: usize,
    db: &Arc<LenaDb>,
) -> Result<()>
```

## `run`

```rust
fn run(
    db: &Arc<LenaDb>,
    fqsn: &str,
    format: &str,
    template_path: Option<&PathBuf>,
) -> Result<()>
```

## `run`

```rust
fn run(db: &Arc<LenaDb>, fqsn: &str) -> Result<()>
```

## `run`

```rust
fn run(
    db: &Arc<LenaDb>,
    out_dir: &PathBuf,
    theme: &str,
    css_path: Option<&PathBuf>,
    github_actions: bool,
) -> Result<()>
```

## `run`

```rust
fn run(db: &Arc<LenaDb>, fqsn: &str) -> Result<()>
```

## `run_fixture`

```rust
fn run_fixture(file: &str) -> String
```

## `run_native_adapter`

```rust
fn run_native_adapter(
    adapter_name: &str,
    target: PathBuf,
    db: Arc<LenaDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize>
```

## `run_python`

```rust
fn run_python() -> Vec<String>
```

## `run_treesitter_directory`

```rust
fn run_treesitter_directory(
    target: PathBuf,
    lang: &str,
    grammar_file: PathBuf,
    db: Arc<LenaDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize>
```

## `run_two_pass_on_fixture`

```rust
fn run_two_pass_on_fixture(path: &Path, content: &str) -> Vec<String>
```

/// Simulates the two-pass architecture for a single file to support unit tests.

## `sanitize_mermaid`

```rust
fn sanitize_mermaid(s: &str) -> String
```

## `setup_db`

```rust
fn setup_db() -> LenaDb
```

## `setup_db`

```rust
fn setup_db() -> Arc<LenaDb>
```

## `standard_function`

```rust
fn standard_function(x: i32) -> bool
```

/// This is a docstring for a standard function.

## `standard_rules`

```rust
fn standard_rules() -> &'static str
```

/// Exposes standard rules for caller-constructed queries.

## `tap_inner_mut`

```rust
fn tap_inner_mut(self, f: F) -> Self
```

## `test_1_based_line_indexing`

```rust
fn test_1_based_line_indexing()
```

## `test_bit_mask_verification`

```rust
fn test_bit_mask_verification()
```

## `test_bootstrap_code_payload_attributes`

```rust
fn test_bootstrap_code_payload_attributes()
```

## `test_bootstrap_graph_edge_attributes`

```rust
fn test_bootstrap_graph_edge_attributes()
```

## `test_bootstrap_identity_attributes`

```rust
fn test_bootstrap_identity_attributes()
```

## `test_bootstrap_location_attributes`

```rust
fn test_bootstrap_location_attributes()
```

## `test_bootstrap_unknown_attribute`

```rust
fn test_bootstrap_unknown_attribute()
```

## `test_chained_method_calls`

```rust
fn test_chained_method_calls()
```

## `test_clj_callsite_mapping`

```rust
fn test_clj_callsite_mapping()
```

## `test_clj_defn_and_private_facts`

```rust
fn test_clj_defn_and_private_facts()
```

## `test_clj_edge_cases_parse`

```rust
fn test_clj_edge_cases_parse()
```

## `test_clj_graceful_empty_handling`

```rust
fn test_clj_graceful_empty_handling()
```

## `test_clj_hash_id_masking`

```rust
fn test_clj_hash_id_masking()
```

## `test_clj_metadata_extraction`

```rust
fn test_clj_metadata_extraction()
```

## `test_clj_module_definition`

```rust
fn test_clj_module_definition()
```

## `test_clj_ring_stress_add_count`

```rust
fn test_clj_ring_stress_add_count()
```

## `test_clj_threading_macro_callsites`

```rust
fn test_clj_threading_macro_callsites()
```

## `test_contains_rule_call`

```rust
fn test_contains_rule_call()
```

## `test_deterministic_edge_ids`

```rust
fn test_deterministic_edge_ids()
```

## `test_docgraph_builder_extraction`

```rust
fn test_docgraph_builder_extraction()
```

## `test_edn_value_formatting`

```rust
fn test_edn_value_formatting()
```

## `test_enterprise_js_calls`

```rust
fn test_enterprise_js_calls()
```

## `test_enterprise_js_symbols`

```rust
fn test_enterprise_js_symbols()
```

## `test_enterprise_python`

```rust
fn test_enterprise_python()
```

## `test_export_prompt_not_found`

```rust
fn test_export_prompt_not_found()
```

## `test_export_prompt_success`

```rust
fn test_export_prompt_success()
```

## `test_extract_i64_with_and_without_colons`

```rust
fn test_extract_i64_with_and_without_colons()
```

## `test_extract_string_with_and_without_colons`

```rust
fn test_extract_string_with_and_without_colons()
```

## `test_format_dot`

```rust
fn test_format_dot()
```

## `test_format_indented_cycle`

```rust
fn test_format_indented_cycle()
```

## `test_format_indented_empty`

```rust
fn test_format_indented_empty()
```

## `test_format_markdown`

```rust
fn test_format_markdown()
```

## `test_format_mermaid`

```rust
fn test_format_mermaid()
```

## `test_format_xml`

```rust
fn test_format_xml()
```

## `test_generate_skeleton_depth_and_query`

```rust
fn test_generate_skeleton_depth_and_query()
```

## `test_graceful_syntax_error_handling`

```rust
fn test_graceful_syntax_error_handling()
```

## `test_html_renderer_emission`

```rust
fn test_html_renderer_emission()
```

## `test_ingest_stream`

```rust
fn test_ingest_stream()
```

## `test_ingest_stream_basic`

```rust
fn test_ingest_stream_basic()
```

## `test_ingest_stream_clean_eof`

```rust
fn test_ingest_stream_clean_eof()
```

## `test_ingest_stream_empty_transactions`

```rust
fn test_ingest_stream_empty_transactions()
```

## `test_ingest_stream_resilience_malformed_edn`

```rust
fn test_ingest_stream_resilience_malformed_edn()
```

## `test_ingest_stream_skips_empty_lines`

```rust
fn test_ingest_stream_skips_empty_lines()
```

## `test_javascript_grammar_loading`

```rust
fn test_javascript_grammar_loading()
```

## `test_lenadb_open_and_rules`

```rust
fn test_lenadb_open_and_rules()
```

## `test_mdbook_emission`

```rust
fn test_mdbook_emission()
```

## `test_mermaid_generation_escapes_quotes`

```rust
fn test_mermaid_generation_escapes_quotes()
```

## `test_missing_optional_captures`

```rust
fn test_missing_optional_captures()
```

## `test_multiple_captures_in_node`

```rust
fn test_multiple_captures_in_node()
```

## `test_python_call_sites`

```rust
fn test_python_call_sites()
```

## `test_python_symbol_captures`

```rust
fn test_python_symbol_captures()
```

## `test_query_json_basic`

```rust
fn test_query_json_basic()
```

## `test_query_with_custom_rules`

```rust
fn test_query_with_custom_rules()
```

## `test_query_with_rules_auto_injects`

```rust
fn test_query_with_rules_auto_injects()
```

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

## `test_schema_bootstrap_contains_all_attributes`

```rust
fn test_schema_bootstrap_contains_all_attributes()
```

## `test_stress_express_application`

```rust
fn test_stress_express_application()
```

## `test_stress_fastapi_routing`

```rust
fn test_stress_fastapi_routing()
```

## `test_symbol_kind_parsing`

```rust
fn test_symbol_kind_parsing()
```

## `test_transaction_builder`

```rust
fn test_transaction_builder()
```

## `test_treesitter_cross_file_resolution`

```rust
fn test_treesitter_cross_file_resolution()
```

## `to_markdown`

```rust
fn to_markdown(&self) -> String
```

## `to_xml`

```rust
fn to_xml(&self) -> String
```

## `traits`

```rust
fn traits()
```

## `visit_dirs`

```rust
fn visit_dirs(dir: &Path) -> Result<()>
```

## `with_state`

```rust
fn with_state(self, state: S) -> Fallback<S2, E>
```

