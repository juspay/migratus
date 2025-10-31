# Migratus - Justfile for common development tasks

# Run clippy linter
clippy:
    cargo clippy --all-targets --all-features

# Format code with nightly formatter
fmt:
    cargo +nightly fmt --all

# Install binary to ~/.cargo/bin
install:
    cargo install --path . --root ~/.cargo

# Run all checks (clippy + fmt check)
check: clippy
    cargo +nightly fmt --all -- --check

# Build release binary
build:
    cargo build --release

# Run tests
test:
    cargo test

# Clean build artifacts
clean:
    cargo clean

# Run the migration tool with a config file
run CONFIG:
    cargo run --release -- {{CONFIG}}

# Show all available commands
help:
    @just --list
