#!/usr/bin/env bash

cd astrolabe-adapter-rust

cargo build --release
cargo test
