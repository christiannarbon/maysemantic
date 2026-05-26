# SDK Parsing and Validation

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
