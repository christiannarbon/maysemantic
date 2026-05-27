use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dwh_secret_serialization() -> Result<(), Box<dyn std::error::Error>> {
        let secret = DwhSecret::UsernamePassword {
            host: "localhost".to_string(),
            port: 5432,
            database: "postgres".to_string(),
            username: "admin".to_string(),
            password: "password".to_string(),
        };

        let json = serde_json::to_string(&secret)?;
        assert!(json.contains("UsernamePassword"));
        assert!(json.contains("localhost"));

        let deserialized: DwhSecret = serde_json::from_str(&json)?;
        match deserialized {
            DwhSecret::UsernamePassword { host, .. } => assert_eq!(host, "localhost"),
            _ => return Err("Expected UsernamePassword variant".into()),
        }

        Ok(())
    }
}
