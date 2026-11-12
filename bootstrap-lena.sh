#!/usr/bin/env bash

set -euo pipefail

echo "==> Cleaning old database"
rm -rf .lena/db

echo "==> Rebuilding Leña"
cargo build
cd lena-adapter-go
go build -o ../target/debug/lena-adapter-go ./src
cd ..

echo "==> Indexing Leña"
cargo run --bin lena -- index -A .

echo "==> Creating Leña Book"
cargo run --bin lena -- doc --out docs
