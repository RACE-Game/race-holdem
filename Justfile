release: build optimize

test:
    cargo test

build:
    cargo build -r --target wasm32-unknown-unknown

optimize:
    wasm-opt -Oz target/wasm32-unknown-unknown/release/race_holdem.wasm -o target/race_holdem.wasm
