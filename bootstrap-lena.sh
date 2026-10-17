#!/usr/bin/env bash

set -euo pipefail

echo "==> Cleaning old database..."
rm -rf .lena/db

echo "==> Indexing Leña"
cargo run --bin lena -- index .

echo "==> Creating Leña Book"
cargo run --bin lena -- doc --out docs
