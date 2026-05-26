use crate::error::AuthError;
use sqlx::PgPool;
use std::env;

/// Ensures the default admin user exists by reading MAY_ADMIN_PASSWORD.
pub async fn ensure_admin(pool: &PgPool) -> Result<(), AuthError> {
    let password = env::var("MAY_ADMIN_PASSWORD").map_err(|_| {
        AuthError::MissingConfig(
            "MAY_ADMIN_PASSWORD must be set to seed the initial admin account".to_string(),
        )
    })?;

    let hash_result =
        tokio::task::spawn_blocking(move || crate::password::hash_password(&password)).await;

    let password_hash = match hash_result {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(AuthError::HashError("Spawn blocking failed".to_string())),
    };

    sqlx::query!(
        r#"
        INSERT INTO users (username, password_hash, role, is_active)
        VALUES ('admin', $1, 'admin'::user_role, true)
        ON CONFLICT (username) DO NOTHING
        "#,
        password_hash
    )
    .execute(pool)
    .await?;

    Ok(())
}
