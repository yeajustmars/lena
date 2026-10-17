# Module `lena-adapter-rust/tests/fixtures/rust/axum/axum/0.7.4/axum/src/routing/mod.rs`

**Source**: `lena-adapter-rust/tests/fixtures/rust/axum/axum/0.7.4/axum/src/routing/mod.rs`

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

## `default`

```rust
fn default() -> Self
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

## `fmt`

```rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
```

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
fn new() -> Self
```

/// Create a new `Router`.
///
/// Unless you add additional routes this will respond with `404 Not Found` to
/// all requests.

## `poll_ready`

```rust
fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
```

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

## `tap_inner_mut`

```rust
fn tap_inner_mut(self, f: F) -> Self
```

## `traits`

```rust
fn traits()
```

## `with_state`

```rust
fn with_state(self, state: S) -> Fallback<S2, E>
```

## `Unnamed`

