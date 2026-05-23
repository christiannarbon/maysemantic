.PHONY: setup build check test fmt lint run clean help

# Default target
help:
	@echo "Available commands:"
	@echo "  make setup   - Configure git hooks and set up the local development environment"
	@echo "  make build   - Build the entire workspace"
	@echo "  make check   - Run formatting, linting, and unit tests (simulates pre-push)"
	@echo "  make test    - Run unit tests across the workspace"
	@echo "  make fmt     - Format Rust code using cargo fmt"
	@echo "  make lint    - Lint Rust code using clippy (denies warnings)"
	@echo "  make run     - Run the PGWire Gateway Service"
	@echo "  make clean   - Clean the cargo build cache"

setup:
	@echo "Setting up local development environment..."
	git config core.hooksPath .githooks
	@echo "Git hooks configured to use .githooks/"

build:
	@echo "Building workspace..."
	cargo build --workspace

fmt:
	@echo "Formatting code..."
	cargo fmt --all

lint:
	@echo "Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

test:
	@echo "🧪 Running unit tests..."
	cargo test --test unit

integration-test:
	@echo "🧪 Running integration tests..."
	cargo test --test integration

check: fmt lint test
	@echo "All checks passed!"

run_pgwire:
	@echo "Starting PGWire Gateway Service..."
	cargo run -p may_pgwire

clean:
	@echo "Cleaning workspace..."
	cargo clean
