# Contributing to May Semantic Layer

First, thank you for your interest in contributing to the `may` open-source semantic layer!

## Development Setup

The `may` project is written in Rust. You will need the standard Rust toolchain installed (`cargo`, `rustc`, `rustfmt`, `clippy`).

```bash
# Clone the repository
git clone https://github.com/your-org/maysemantic.git
cd maysemantic

# Run the tests to ensure everything is working locally
cargo test --workspace
```

## Continuous Integration (CI) Checks

We enforce a strict CI pipeline on all Pull Requests to maintain high code quality and prevent regressions. The pipeline runs the following checks:

### 1. Formatting
All code must be formatted using the standard `rustfmt` tool.
**How to check locally:**
```bash
cargo fmt --all -- --check
```
**How to fix:**
```bash
cargo fmt --all
```

### 2. Linting (Clippy)
We require all code to be warning-free. We use `clippy` to catch common mistakes and enforce idiomatic Rust.
**How to check locally:**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### 3. Testing
All unit and integration tests must pass.
**How to run locally:**
```bash
cargo test --workspace
```

## Submitting a Pull Request

1. Fork the repository.
2. Create a new branch for your feature or bugfix.
3. Make your changes, ensuring you write tests for any new logic.
4. Run the CI checks locally (`fmt`, `clippy`, `test`).
5. Commit your changes and push to your fork.
6. Open a Pull Request against the `main` branch.

Our GitHub Actions pipeline will run automatically and report any issues!
