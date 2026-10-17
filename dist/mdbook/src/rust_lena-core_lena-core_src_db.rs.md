# Module `lena-core/src/db.rs`

**Source**: `lena-core/src/db.rs`

## `contains_rule_call`

```rust
fn contains_rule_call(query: &str) -> bool
```

/// Helper to detect if a query string invokes standard recursive rule predicates.

## `inner`

```rust
fn inner(&self) -> &MesoDb
```

/// Access the underlying MesoDB engine directly.

## `open`

```rust
fn open(path: impl AsRef<Path>) -> Result<Self, DbError>
```

/// Opens or creates an Lena database at the specified path.
/// # Errors
/// Returns `DbError::MesoDb` if the underlying database engine fails.

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

## `setup_db`

```rust
fn setup_db() -> LenaDb
```

## `standard_rules`

```rust
fn standard_rules() -> &'static str
```

/// Exposes standard rules for caller-constructed queries.

## `test_contains_rule_call`

```rust
fn test_contains_rule_call()
```

## `test_lenadb_open_and_rules`

```rust
fn test_lenadb_open_and_rules()
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

## `Unnamed`

