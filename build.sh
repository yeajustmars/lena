#!/usr/bin/env bash

echo -e "[INFO] Building Go adapter"
cd astrolabe-adapter-go
go build -o ../target/debug/astrolabe-adapter-go ./src
cd ..

echo -e "[INFO] Building Rust adapter"
cargo build
