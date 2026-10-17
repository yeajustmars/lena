# Module `lena-core/src/exporter.rs`

**Source**: `lena-core/src/exporter.rs`

## `collect_files`

```rust
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()>
```

## `export_directory`

```rust
fn export_directory(dir: &Path, format: ExportFormat) -> Result<String, ExporterError>
```

/// Exports an entire directory into a formatted string (Markdown or XML).

## `export_prompt`

```rust
fn export_prompt(
    db: &Arc<LenaDb>,
    fqsn: &str,
    format: ExportFormat,
) -> Result<String, ExporterError>
```

/// Retrieves an entity by its FQSN and formats it for LLM context.

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

## `test_format_markdown`

```rust
fn test_format_markdown()
```

## `test_format_xml`

```rust
fn test_format_xml()
```

## `to_markdown`

```rust
fn to_markdown(&self) -> String
```

## `to_xml`

```rust
fn to_xml(&self) -> String
```

## `Unnamed`

