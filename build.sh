#!/bin/sh

# build hello plugin

cd crates/lyserver_hello_plugin
cargo build --release --target wasm32-wasip1
cp static/manifest.toml ../../server_data/plugins/lyserver_hello_plugin/
cd ../../
mkdir -p ./server_data/plugins/lyserver_hello_plugin
cp target/wasm32-wasip1/release/lyserver_hello_plugin.wasm ./server_data/plugins/lyserver_hello_plugin/

# build media plugin

cd crates/lyserver_media_plugin
cargo build --release --target wasm32-wasip1
cp static/manifest.toml ../../server_data/plugins/lyserver_media_plugin/
cd ../../
mkdir -p ./server_data/plugins/lyserver_media_plugin
cp target/wasm32-wasip1/release/lyserver_media_plugin.wasm ./server_data/plugins/lyserver_media_plugin/

# build + run

RUST_BACKTRACE=1 RUST_LOG=debug,lyserver=debug,actix_web::middleware::Logger=debug cargo run -- --data-dir ./server_data