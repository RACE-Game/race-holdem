release: build optimize

test:
    cargo test

build:
    cargo build -r --target wasm32-unknown-unknown -p race-holdem-base

optimize:
    wasm-opt -Oz target/wasm32-unknown-unknown/release/race_holdem_base.wasm -o target/race_holdem_base.wasm
