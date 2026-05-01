# MCP macOS Calendar — Justfile
# https://github.com/casey/just

# Default recipe (listed when running `just`)
default:
    @just --list

# Build the project in debug mode
build:
    cargo build

# Build the project in release mode
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run unit tests only
test-unit:
    cargo test --lib

# Run integration tests only
test-integration:
    cargo test --test integration_main

# Run the server in stdio mode
run-stdio:
    cargo run -- --transport stdio

# Run the server in SSE mode (default port 8080)
run-sse port="8080" host="127.0.0.1":
    cargo run -- --transport sse --port {{port}} --host {{host}}

# Run the server in SSE mode on port 3000
run-sse-3000:
    cargo run -- --transport sse --port 3000

# Check the project for errors without building
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Run linter
lint:
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    cargo clean
