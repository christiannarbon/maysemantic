use crate::error::AuthError;
use crate::models::User;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
}

pub struct TokenService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiry_secs: usize,
}

impl TokenService {
    pub fn new() -> Result<Self, AuthError> {
        let secret = env::var("MAY_JWT_SECRET")
            .map_err(|_| AuthError::MissingConfig("MAY_JWT_SECRET".to_string()))?;
        let expiry_secs = env::var("MAY_JWT_EXPIRY_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            expiry_secs,
        })
    }

    pub fn issue(&self, user: &User) -> Result<String, AuthError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AuthError::InvalidToken)?
            .as_secs() as usize;

        let claims = Claims {
            sub: user.id.to_string(),
            role: format!("{:?}", user.role).to_lowercase(),
            exp: now + self.expiry_secs,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|_| AuthError::InvalidToken)
    }

    pub fn verify(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::default();
        validation.leeway = 0;

        let token_data = decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| {
            match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                _ => AuthError::InvalidToken,
            }
        })?;

        Ok(token_data.claims)
    }
}
