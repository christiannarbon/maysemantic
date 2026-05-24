# Supported Dialects

The `may` semantic layer translates an agnostic intermediate AST (`SqlNode`) into dialect-specific SQL for multiple target data warehouses. This ensures that semantic models write once and run everywhere.

## Currently Supported Warehouses

1. **PostgreSQL (`PostgresDialect`)**
   - Identifier Quoting: Double quotes (`"users"`)
   - Temporal: Standard ANSI (`DATE_TRUNC('month', created_at)`)
   - JSON Extraction: Planned via `->>`
   - Array Unnesting: Planned via `UNNEST()`

2. **Snowflake (`SnowflakeDialect`)**
   - Identifier Quoting: Double quotes, but implicitly converts identifiers to uppercase (`"USERS"`) to respect Snowflake's case-insensitive unquoted matching behavior.
   - Temporal: Standard ANSI with uppercase granularity (`DATE_TRUNC('MONTH', created_at)`)
   - JSON Extraction: Uses `GET_PATH(column, 'path')`
   - Array Unnesting: Planned via `FLATTEN()`

3. **Google BigQuery (`BigQueryDialect`)**
   - Identifier Quoting: Backticks (`` `users` ``)
   - Temporal: Reversed arguments with unquoted granularity (`DATE_TRUNC(created_at, MONTH)`)
   - JSON Extraction: Planned via `JSON_EXTRACT_SCALAR`
   - Array Unnesting: Uses `UNNEST(array_column)`

## Contributing a New Dialect

Adding a new dialect is straightforward. All dialect generation logic lives in the `dialects` module under `may_core/src/`. 

To implement a new warehouse target (e.g., `DatabricksDialect`), follow these steps:

### 1. Create the Adapter Struct
Create a new file in `may_core/src/dialects/` (e.g., `may_core/src/dialects/databricks.rs`) and define a struct for your dialect.

```rust
use crate::dialects::{DialectError, SqlDialect};

#[derive(Debug)]
pub struct DatabricksDialect;
```

### 2. Implement the `SqlDialect` Trait
Implement the `SqlDialect` trait for your struct. By default, `SqlDialect` provides standard ANSI SQL generation. You **only** need to override methods where your target warehouse differs from standard ANSI.

The most common overrides are:
- `quote_identifier`: How does the warehouse quote identifiers? (e.g., `"`, ``` `` ```, `[]`).
- `write_date_trunc`: How does the warehouse handle temporal truncation?

**CRITICAL RULE: Zero Allocation**
All `write_*` methods in the trait accept a `&mut String` buffer. You MUST write your output directly into this buffer using `buf.push()`, `buf.push_str()`, or the `write!` macro. Do NOT allocate intermediate `String` objects on the heap.

```rust
impl SqlDialect for DatabricksDialect {
    fn quote_identifier(&self, ident: &str) -> String {
        // Implement your custom quoting logic here (must return a new String per the trait)
        format!("`{}`", ident) 
    }

    fn write_date_trunc(
        &self,
        buf: &mut String,
        granularity: &str,
        column: &str,
    ) -> Result<(), DialectError> {
        // Example: write directly to the buffer to avoid allocation
        buf.push_str("DATE_TRUNC('");
        for c in granularity.chars() {
            buf.push(c.to_ascii_uppercase());
        }
        buf.push_str("', ");
        let quoted = self.quote_identifier(column);
        buf.push_str(&quoted);
        buf.push(')');
        Ok(())
    }
}
```

### 3. Add Custom Helpers (Optional)
If your warehouse has specific functions that aren't yet represented broadly in the AST (like JSON extraction or specific array unnesting), implement them as public methods on your struct (e.g., `write_json_extract`).

### 4. Register and Test
- Register your module in `may_core/src/dialects/mod.rs` and re-export the struct.
- Create a test file in `may_core/tests/unit/dialects/` (e.g., `may_core/tests/unit/dialects/databricks_test.rs`).
- Write integration tests to ensure your overrides generate the expected SQL.
- Run `cargo fmt`, `cargo clippy`, and `cargo test`.
