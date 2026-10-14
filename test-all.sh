#!/usr/bin/env bash

# Exit immediately if a command exits with a non-zero status
set -e

echo "=== Running Rust Workspace Linters & Tests ==="
# Enforce the strict Clippy rules across the entire workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
# Run all Rust tests
cargo test --workspace

echo ""
echo "=== Running Go Adapter Tests ==="
cd lena-adapter-go
go test -v ./...
cd ..

echo ""
echo "✅ All tests and linters passed successfully!"
