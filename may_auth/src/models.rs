use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, PartialEq, Eq)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum Role {
    Admin,
    Viewer,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_admin_seed_hash() {
        let hash = "$2b$12$q/phIyk4Tw0VsbiBWgo29ea2iUQ031y7gpJNhri4P/pljfEKX4YPq";
        let valid = bcrypt::verify("changeme", hash);
        assert!(
            matches!(valid, Ok(true)),
            "The bcrypt hash should match 'changeme'"
        );
    }
}
