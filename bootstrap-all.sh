#!/usr/bin/env bash

set -euo pipefail

echo "==> Cleaning old database..."
rm -rf .lena/db

echo "==> Indexing Leña"
cargo run --bin lena -- index .

echo "==> Creating Leña Book"
cargo run --bin lena -- doc --out lena-book

echo "==> Generating single-page Leña docs"
cargo run --bin lena -- doc --theme single-page --out dist/single

echo "==> Creating mkbook for Leña"
cargo run --bin lena -- doc --theme mdbook --out dist/mdbook
