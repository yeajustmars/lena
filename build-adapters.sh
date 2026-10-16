#!/usr/bin/env bash

set -euo pipefail

echo "==> Building Rust workspace (lena-cli, lena-adapter-rust, lena-adapter-clj, lena-adapter-treesitter)..."
cargo build

# Ensure the debug directory exists (cargo build should have created this)
mkdir -p target/debug

echo "==> Building Go adapter..."
cd lena-adapter-go
# Compile the Go adapter and output the binary directly into the Rust target dir
go build -o ../target/debug/lena-adapter-go src/main.go
cd ..

echo "==> Done! All adapters are staged in target/debug/"
