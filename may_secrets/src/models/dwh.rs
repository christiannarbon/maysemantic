use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DwhSecret {
    UsernamePassword {
        host: String,
        port: u16,
        database: String,
        username: String,
        password: String,
    },
    ServiceAccountKey {
        json: String,
    },
    KeyPair {
        account: String,
        username: String,
        private_key: String,
        passphrase: Option<String>,
    },
}
