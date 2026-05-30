use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn default_vault_mount() -> String {
    "secret".to_string()
}

fn default_cache_ttl_secs() -> u64 {
    300
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SecretsMode {
    Managed,
    Byov,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VaultAuthMethod {
    Token,
    AppRole,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct MaySecretsConfig {
    pub mode: SecretsMode,

    pub vault_address: Option<String>,

    #[serde(default = "default_vault_mount")]
    pub vault_mount: String,

    pub auth_method: VaultAuthMethod,

    pub token: Option<String>,

    pub role_id: Option<String>,

    pub secret_id: Option<String>,

    #[serde(default = "default_cache_ttl_secs")]
    pub cache_ttl_secs: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/config.rs");

    let schema = schemars::schema_for!(MaySecretsConfig);
    let schema_json = serde_json::to_string_pretty(&schema)?;

    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let mut path = PathBuf::from(manifest_dir);
    path.pop(); // Go up to workspace root
    path.push("docs");
    fs::create_dir_all(&path)?;
    path.push("may_secrets.schema.json");

    fs::write(path, schema_json)?;

    Ok(())
}
