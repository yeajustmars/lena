# Module `lena-core/src/docgen/html.rs`

**Source**: `lena-core/src/docgen/html.rs`

## `new`

```rust
fn new(graph: &'a DocGraph, custom_css: Option<String>) -> Result<Self>
```

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

## `test_html_renderer_emission`

```rust
fn test_html_renderer_emission()
```

## `Unnamed`

