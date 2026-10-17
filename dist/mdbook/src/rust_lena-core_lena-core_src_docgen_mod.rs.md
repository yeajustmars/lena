# Module `lena-core/src/docgen/mod.rs`

**Source**: `lena-core/src/docgen/mod.rs`

## `build`

```rust
fn build(self) -> Result<DocGraph>
```

## `extract_i64`

```rust
fn extract_i64(val: &Value, key: &str) -> i64
```

## `extract_string`

```rust
fn extract_string(val: &Value, key: &str) -> String
```

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

## `test_docgraph_builder_extraction`

```rust
fn test_docgraph_builder_extraction()
```

