#!/usr/bin/env bash

cd lena-adapter-rust

cargo build --release
cargo test
