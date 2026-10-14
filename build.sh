#!/usr/bin/env bash

echo -e "[INFO] Building Go adapter"
cd lena-adapter-go
go build -o ../target/debug/lena-adapter-go ./src
cd ..

echo -e "[INFO] Building Rust adapter"
cargo build
