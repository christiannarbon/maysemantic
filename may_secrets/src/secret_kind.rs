use crate::error::SecretsError;
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub(crate) enum SecretKind {
    UsernamePassword,
    ServiceAccountKey,
    KeyPair,
}

impl FromStr for SecretKind {
    type Err = SecretsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "username_password" => Ok(Self::UsernamePassword),
            "service_account_key" => Ok(Self::ServiceAccountKey),
            "key_pair" => Ok(Self::KeyPair),
            _ => Err(SecretsError::InvalidConfig(format!(
                "Unknown secret type: {s}"
            ))),
        }
    }
}
