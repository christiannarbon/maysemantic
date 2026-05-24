# May Semantic Layer

[![CI](https://github.com/christiannarbon/may_core/actions/workflows/ci.yml/badge.svg)](https://github.com/christiannarbon/may_core/actions)
[![Last Commit](https://img.shields.io/github/last-commit/christiannarbon/may_core?style=flat-square)](https://github.com/christiannarbon/may_core/commits/main)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg?style=flat-square)](https://github.com/christiannarbon/may_core)
[![Rustc](https://img.shields.io/badge/rust-1.75+-lightgray.svg?style=flat-square)](https://blog.rust-lang.org)

The `may_core` crate provides the core State Manager (`StateMgr`) and generic semantic models for the `may` open-source semantic layer.

## CLI Application

A unified interface for Analytics Engineers to locally validate and compile their semantic definitions.

### Installation

To install the `may` CLI tool locally, run:
```bash
cargo install --path cli
```

### Usage

Once installed, you can use the CLI from anywhere:

```bash
# Display help and subcommands
may --help

# Validate models in the current directory (Outputs in Green/Red)
may validate

# Validate models in a specific directory
may validate --path ./my_models

# Run a semantic query (simulation)
may run --query "Revenue by Region"

# Compile models into optimized SQL
may compile
```

> **Note on Terminal Output**: The `may` CLI uses color-coded output to enhance readability. Successful validations appear in **Green**, while parsing or syntax errors appear in **Red** with detailed file paths and error reasons.

## JSON Schema Generation

To provide autocomplete and validation for VS Code or other editors when editing `metrics.yml` files, you can generate the JSON Schema by running:

```bash
cargo run --bin generate_schema > metrics.schema.json
```

Once generated, you can use the schema in your YAML files to get IDE assistance:
```yaml
# yaml-language-server: $schema=./metrics.schema.json

name: ecommerce_model
entities:
  # ...
```

## Parsing and Validation

The `StateMgr` handles safely loading and validating YAML definitions into strict memory-safe Rust models.
Invalid inputs trigger strict regex and `serde` validations dynamically without panicking.

```rust
use may_core::StateMgr;

let state_mgr = StateMgr::new();
match state_mgr.load_from_yaml(include_str!("../metrics.yml")) {
    Ok(_) => println!("Successfully loaded metrics!"),
    Err(e) => eprintln!("Failed to load metrics: {}", e),
}
```
