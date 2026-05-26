.PHONY: setup build check test fmt lint run clean help

# Default target
help:
	@echo "Available commands:"
	@echo "  make setup   - Configure git hooks and set up the local development environment"
	@echo "  make build   - Build the entire workspace"
	@echo "  make check   - Run formatting, linting, and unit tests (simulates pre-push)"
	@echo "  make test    - Run unit tests across the workspace"
	@echo "  make fmt     - Format Rust code using cargo fmt"
	@echo "  make lint       - Lint Rust code using clippy (denies warnings)"
	@echo "  make run        - Run the PGWire Gateway Service"
	@echo "  make clean      - Clean the cargo build cache"
	@echo "  make docker-up  - Start Docker Compose deployment"
	@echo "  make docker-down- Stop Docker Compose deployment"

setup:
	@echo "Setting up local development environment..."
	git config core.hooksPath .githooks
	@echo "Git hooks configured to use .githooks/"
	@if [ ! -f deployments/docker-compose/.env ]; then \
		cp deployments/docker-compose/.env.example deployments/docker-compose/.env; \
		echo "Created .env from .env.example. Please review the variables in deployments/docker-compose/.env:"; \
		echo "  - IDENTITY_DB_USER"; \
		echo "  - IDENTITY_DB_PASSWORD"; \
		echo "  - IDENTITY_DB_NAME"; \
		echo "  - MAY_ADMIN_PASSWORD"; \
		echo "  - MAY_JWT_SECRET"; \
	else \
		echo ".env file already exists. Please ensure it contains the required variables from .env.example."; \
	fi

docker-up:
	@echo "Starting Docker Compose services..."
	docker compose -f deployments/docker-compose/docker-compose.yml up -d

docker-down:
	@echo "Stopping Docker Compose services..."
	docker compose -f deployments/docker-compose/docker-compose.yml down

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
