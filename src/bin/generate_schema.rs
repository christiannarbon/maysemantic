use maysemantic::models::SemanticModel;

fn main() {
    let schema = schemars::schema_for!(SemanticModel);
    let schema_json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    println!("{schema_json}");
}
