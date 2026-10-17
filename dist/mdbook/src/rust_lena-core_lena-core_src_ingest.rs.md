# Module `lena-core/src/ingest.rs`

**Source**: `lena-core/src/ingest.rs`

## `ingest_stream`

```rust
fn ingest_stream(db: &Arc<LenaDb>, reader: R) -> Result<usize, IngestError>
```

/// Ingests ND-EDN lines from an asynchronous reader (like an adapter's stdout)
/// and streams them directly into the database.
///
/// # Errors
/// Returns `IngestError` if the underlying reader fails or if a transaction panics.

## `setup_db`

```rust
fn setup_db() -> Arc<LenaDb>
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

## `Unnamed`

