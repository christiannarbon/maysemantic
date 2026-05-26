# CLI Application Usage

A unified interface for Analytics Engineers to locally validate and compile their semantic definitions.

## Installation

To install the `may` CLI tool locally, run:
```bash
cargo install --path cli
```

## Usage

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
