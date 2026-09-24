default: check

build:
    cargo build --workspace --all-targets

test:
    cargo test --workspace

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo run --quiet -p xtask -- lint-comments
    cargo run --quiet -p xtask -- lint-docs
    cargo run --quiet -p xtask -- lint-prose

replay commits="200":
    cargo run --quiet --release -p xtask -- replay --commits {{commits}}

check: lint test
    cargo run --quiet --release -p xtask -- replay --commits 200
    cargo deny check
    cargo build --release --bin codedoc
    ./target/release/codedoc doctor

install:
    cargo install --path crates/codedoc-cli
    cargo install --path crates/codedoc-mcp
    cargo install --path crates/codedoc-lsp

bench root:
    cargo build --release -p codedoc-cli
    ./target/release/codedoc --root {{root}} verify
