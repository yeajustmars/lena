# Leña 🪵

> A graph-backed Swiss Army knife for understanding code, exploring call relationships, building clean documentation, and giving language models precise context.

Modern codebases are graphs, but we usually look at them through flat file trees. **Leña** parses multi-language codebases into an embedded graph database, connecting symbols, modules, files, and call relationships together so you can see how your project actually fits together.

Whether you want to generate documentation with interactive call graphs, export deterministic context windows for LLMs, trace transitive callers across a project, or audit a legacy enterprise codebase without breaking a sweat, Leña gives you the tools without heavyweight infrastructure.

---

## 🌟 What Can Leña Do?

### 1. Resilient & Concurrent Graph Ingestion (`lena index`)
Leña runs lightweight, per-file isolated language adapters that extract AST symbols, locations, docstrings, and call sites. These are streamed as ND-EDN facts directly into an embedded [**MesoDB**](https://github.com/yeajustmars/mesodb) datalog engine.
* **Concurrent Execution:** Powered by Tokio and `indicatif`, Leña orchestrates multi-language indexing pipelines concurrently with live sub-second progress spinners.
* **Enterprise Resilience:** If an adapter panics on a malformed file, the error is isolated. Leña intercepts `@DIAG:` and `@PROG:` coordinates via `stderr` sidecars to keep the pipeline moving while logging the anomaly.
* **Non-Destructive Testing:** Use `lena index --dry-run` or `lena check` to safely validate massive codebases before mutating your graph.

### 2. Beautiful, Modern Static Documentation (`lena doc`)
Turn your indexed codebase into clean, searchable static documentation with a single command:
* **Interactive Call Neighborhoods:** Every documented function or struct automatically generates an interactive [Mermaid.js](https://mermaid.js.org/) graph showing who calls it and what it calls.
* **Instant Search:** An embedded Web Worker powers sub-millisecond, offline prefix-trie searching.
* **Multi-Format Export:** Generate a multi-page `lena-book`, a single-page standalone HTML file, or an `mdbook`.
* **Theming:** Includes 5 built-in developer themes (Grayscale, Navy, Green, Poimandres, Gruvbox) with automatic OS-level light/dark mode detection.

### 3. Precise LLM Context Windows (`lena prompt`)
Stop dumping thousands of tokens of irrelevant source code into chat windows. With `lena prompt`, target a specific Fully Qualified Symbol Name (FQSN) to extract a surgical prompt bundle containing:
* The symbol’s exact source location, signature, and docstrings.
* A resolved list of callers and callees pulled directly from the Datalog graph.
* Output formatted as clean XML or Markdown, perfectly budgeted via a token guardrail.

### 4. Deterministic Code Snapshots (`lena snapshot`)
When you *do* need full directory context for an LLM, standard markdown snippets often break when embedded files contain backticks. Leña walks your directories (respecting `.gitignore`, `target/`, and vendor directories) and bundles files safely into CDATA-wrapped XML or Markdown.

### 5. Call Graph Skeletons (`lena skeleton`)
Trace recursive call graphs from any symbol down to a bounded depth. Export the trace as:
* An indented ASCII tree for quick terminal inspection.
* A Mermaid diagram ready to paste into documentation.
* A Graphviz DOT graph for deep architectural analysis.

---

## 🛠️ Architecture & Components

The Leña ecosystem is organized into three primary layers:

### The Protocol (`lena-protocol`)
The heartbeat of the system. Defines the **Universal Intermediate Representation (UIR)** schema used across all languages.
* **`schema.rs`:** Strict string vocabularies for Entity Types (`:type/module`, `:type/function`), Symbol Kinds (`:kind/struct`, `:kind/interface`), and Graph Edges (`:call/caller`, `:parent/entity`).
* **`validate.rs`:** The `FactValidator` enforces strict invariant boundaries (e.g., call sites must have both a caller and callee; symbol names cannot contain file extensions).
* **`emitter.rs`:** Provides a `TransactionBuilder` for adapters to construct perfectly formatted ND-EDN strings without dragging in a heavy database runtime.

### The Adapters (`lena-adapter-*`)
Language-specific parsers that traverse source files and emit EDN over `stdout` and diagnostic logs over `stderr`.
* **`lena-adapter-rust`:** Deep AST traversal powered by `rust-analyzer` (ra_ap_syntax).
* **`lena-adapter-clojure`:** Integrates with `clj-kondo` for macro-aware, enterprise Lisp parsing.
* **`lena-adapter-go`:** Uses native `go/parser` and `go/ast` to resolve cross-package symbols.
* **`lena-adapter-treesitter`:** A universal polyglot fallback executing `.scm` queries for JavaScript, TypeScript, Python, Ruby, PHP, Java, and Bash.

### The Engine & CLI (`lena-core` / `lena-cli`)
The orchestrator. `lena-cli` handles terminal UX, arguments, and progress bars, delegating heavy lifting to `lena-core`.
* **`db.rs` / `rules.rs`:** Wraps [MesoDB](https://github.com/yeajustmars/mesodb), injecting recursive Datalog rules (like `calls_transitive`) dynamically into user queries.
* **`ingest.rs`:** Asynchronously reads adapter streams, deduplicates FQSN collisions on the fly, validates facts, and commits them to the graph.
* **`report.rs`:** Accumulates adapter timings, fact counts, and `@DIAG:` errors, serializing them into 7 distinct formats (Log, JSON, NDJSON, EDN, Markdown, XML) at the end of a run.
* **`exporter.rs`:** Handles bounded subgraph traversal for `lena prompt` and `lena snapshot`.
* **`docgen/`:** MiniJinja-powered static site generator that compiles the graph into HTML and CSS.

---

## 🚀 Quickstart

### Installation

Install `lena` and the language adapters via `cargo`:

```bash
cargo install lena
cargo install lena-adapter-rust
cargo install lena-adapter-clojure
cargo install lena-adapter-treesitter

```

*(Note: The Go adapter requires a local `go install`—run `lena adapter install go` to handle this automatically).*

### Building from Source

```bash
git clone [https://github.com/yourusername/lena.git](https://github.com/yourusername/lena.git)
cd lena
cargo build --release

```

---

## 📖 Practical Examples

### 1. Indexing a Project

Run Leña against your project root to scan files and populate the local graph database (`.lena/db`).

```bash
# Index with all registered adapters
lena index -A .

# Index safely without writing to the database
lena index -A --dry-run .

```

*At the end of the run, an Indexing Report will be saved to `.lena/reports/`.*

### 2. Generating Documentation

Turn the indexed graph into a static site.

```bash
# Generate the book into docs/ (default)
lena doc --theme lena-book --out docs/

# Or generate a single self-contained HTML file
lena doc --format single-page --out docs/

```

### 3. Discovering Symbols

To assemble context, you need a *Fully Qualified Symbol Name (FQSN)*. Explore the indexed database interactively:

```bash
# Filter symbols using a regular expression
lena list --filter "^rust:.*::test_"
lena list -f "go:edgecases"

```

### 4. Exporting Context for an LLM Prompt

Target a specific function or struct and get its entire immediate call neighborhood formatted for an AI model:

```bash
# Export structured XML (Token budgeted up to depth 2)
lena prompt rust:crate::exporter::export_prompt --format xml --depth 2

# Export Markdown
lena prompt rust:crate::exporter::export_prompt --format markdown

```

### 5. Taking a Directory Snapshot

Pack an entire directory tree into a clean XML bundle to feed into an LLM:

```bash
lena snapshot lena-core/src --format xml --out target/core-snapshot.xml

```

### 6. Visualizing a Call Skeleton

Generate a bounded call graph starting from a root entry point:

```bash
# Trace down 3 levels and print as an indented terminal tree
lena skeleton "rust:crate::main::main" --depth 3 --format indented

# Trace up (who calls this?) and output Mermaid
lena skeleton "rust:crate::db::LenaDb::query_json" --depth 3 --format mermaid --by-callee

```

---

## 🤝 Contributing

Leña is open source and built to be approachable. If you would like to add a language adapter (the `lena-protocol` makes it trivial!), tweak the documentation templates, or improve graph queries, pull requests and issues are very welcome!

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/cool-idea`)
3. Commit your changes (`git commit -m 'Add cool idea'`)
4. Push to the branch (`git push origin feature/cool-idea`)
5. Open a Pull Request
