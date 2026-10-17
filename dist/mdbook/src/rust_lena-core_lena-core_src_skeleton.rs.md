# Module `lena-core/src/skeleton.rs`

**Source**: `lena-core/src/skeleton.rs`

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

## `format_skeleton`

```rust
fn format_skeleton(root: &str, edges: &[Edge], format: SkeletonFormat) -> String
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

## `test_format_mermaid`

```rust
fn test_format_mermaid()
```

## `test_generate_skeleton_depth_and_query`

```rust
fn test_generate_skeleton_depth_and_query()
```

