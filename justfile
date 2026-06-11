# Migratus - Justfile for common development tasks

# Run clippy linter
clippy:
    cargo clippy --all-targets --all-features

# Format code with nightly formatter
fmt:
    cargo +nightly fmt --all

# Build release binaries, install migratus locally, and update ~/.cargo/bin
install: build
    cp target/release/migratus ./migratus
    chmod +x ./migratus
    mkdir -p ~/.cargo/bin
    cp target/release/migratus ~/.cargo/bin/migratus
    cp target/release/updatus ~/.cargo/bin/updatus
    chmod +x ~/.cargo/bin/migratus ~/.cargo/bin/updatus

# Quickly build debug binaries
build-dev:
    cargo build --bins

# Run all checks (clippy + fmt check)
check: clippy
    cargo +nightly fmt --all -- --check

# Build release binary
build:
    cargo build --release --bins

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
