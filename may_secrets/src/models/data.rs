use crate::error::SecretsError;
use crate::models::dwh::DwhSecret;
use crate::models::kind::SecretKind;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize)]
pub(crate) struct UsernamePasswordData {
    pub(crate) host: String,
    #[serde(deserialize_with = "deserialize_port")]
    pub(crate) port: u16,
    pub(crate) database: String,
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Deserialize)]
pub(crate) struct ServiceAccountKeyData {
    pub(crate) json: String,
}

#[derive(Deserialize)]
pub(crate) struct KeyPairData {
    pub(crate) account: String,
    pub(crate) username: String,
    pub(crate) private_key: String,
    pub(crate) passphrase: Option<String>,
}

fn deserialize_port<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if let Some(n) = value.as_u64() {
        u16::try_from(n).map_err(serde::de::Error::custom)
    } else if let Some(s) = value.as_str() {
        s.parse::<u16>().map_err(serde::de::Error::custom)
    } else {
        Err(serde::de::Error::custom("port must be a number or string"))
    }
}

pub(crate) fn build_dwh_secret(kind: SecretKind, data: Value) -> Result<DwhSecret, SecretsError> {
    match kind {
        SecretKind::UsernamePassword => {
            let parsed: UsernamePasswordData = serde_json::from_value(data).map_err(|e| {
                SecretsError::InvalidConfig(format!("Failed to parse UsernamePassword secret: {e}"))
            })?;
            Ok(DwhSecret::UsernamePassword {
                host: parsed.host,
                port: parsed.port,
                database: parsed.database,
                username: parsed.username,
                password: parsed.password,
            })
        }
        SecretKind::ServiceAccountKey => {
            let parsed: ServiceAccountKeyData = serde_json::from_value(data).map_err(|e| {
                SecretsError::InvalidConfig(format!(
                    "Failed to parse ServiceAccountKey secret: {e}"
                ))
            })?;
            Ok(DwhSecret::ServiceAccountKey { json: parsed.json })
        }
        SecretKind::KeyPair => {
            let parsed: KeyPairData = serde_json::from_value(data).map_err(|e| {
                SecretsError::InvalidConfig(format!("Failed to parse KeyPair secret: {e}"))
            })?;
            Ok(DwhSecret::KeyPair {
                account: parsed.account,
                username: parsed.username,
                private_key: parsed.private_key,
                passphrase: parsed.passphrase,
            })
        }
    }
}
