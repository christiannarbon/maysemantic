# May Semantic Layer

[![CI](https://github.com/christiannarbon/maysemantic/actions/workflows/ci.yml/badge.svg)](https://github.com/christiannarbon/maysemantic/actions)
[![Last Commit](https://img.shields.io/github/last-commit/christiannarbon/maysemantic?style=flat-square)](https://github.com/christiannarbon/maysemantic/commits/main)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg?style=flat-square)](https://github.com/christiannarbon/may_core)
[![Rustc](https://img.shields.io/badge/rust-1.75+-lightgray.svg?style=flat-square)](https://blog.rust-lang.org)

`may` is basically a passion project of mine which is basically a semantic layer package. It is a work in progress and is not yet ready for production use. But my goal is that it should be able to support multiple data warehouses like BigQuery, Snowflake, and Postgres. In the long run, I'm hoping to also be able to support other data warehouses like Databricks and Redshift.

## Documentation & Usage

We maintain all detailed usage instructions, tutorials, and wiki documentation in the `docs/` directory.

- **[CLI Usage & Installation](docs/usage/cli.md)**: How to install and use the `may` command-line tool.
- **[IDE Integration & Schema](docs/usage/schema.md)**: How to generate JSON schemas for VS Code autocomplete in `metrics.yml`.
- **[SDK & Core Parsing](docs/usage/sdk.md)**: How to embed the `StateMgr` safely in your Rust applications.
