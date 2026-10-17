# Module `crates/astrolabe-protocol/src/emitter.rs`

**Source**: `crates/astrolabe-protocol/src/emitter.rs`

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

## `finish`

```rust
fn finish(mut self) -> String
```

/// Returns the finalized ND-EDN transaction string.

## `fmt`

```rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
```

## `new`

```rust
fn new() -> Self
```

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

## `test_edn_value_formatting`

```rust
fn test_edn_value_formatting()
```

## `test_transaction_builder`

```rust
fn test_transaction_builder()
```

## `Unnamed`

