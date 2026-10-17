# Module `lena-cli/src/index.rs`

**Source**: `lena-cli/src/index.rs`

## `resolve_binary`

```rust
fn resolve_binary(adapter_name: &str) -> String
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

## `run_native_adapter`

```rust
fn run_native_adapter(
    adapter_name: &str,
    target: PathBuf,
    db: Arc<LenaDb>,
    _permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize>
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

## `Unnamed`

