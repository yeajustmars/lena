# Module `lena-adapter-clj/src/main.rs`

**Source**: `lena-adapter-clj/src/main.rs`

## `execute_kondo`

```rust
fn execute_kondo(target: &Path) -> anyhow::Result<Vec<String>>
```

## `process_kondo_output`

```rust
fn process_kondo_output(kondo_data: KondoOutput) -> anyhow::Result<Vec<String>>
```

/// Core processing logic extracted to allow isolated unit testing of the EDN generation

## `run_fixture`

```rust
fn run_fixture(file: &str) -> String
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

