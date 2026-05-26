# JSON Schema Generation

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
