#!/usr/bin/env bash

set -euo pipefail

echo "==> Indexing Leña"
cargo run --bin lena -- index .

echo "==> Creating Leña Book"
cargo run --bin lena -- doc --out lena-book
