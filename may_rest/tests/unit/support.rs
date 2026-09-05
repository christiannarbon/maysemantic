use may_auth::{
    error::AuthError,
    models::{Role, User},
    repository::UserRepository,
    token::TokenService,
};
use may_rest::AppState;
use std::sync::{Arc, Once};

static INIT_JWT_SECRET: Once = Once::new();

/// Sets the JWT secret once per test binary.
///
/// NOT fully sound: `set_var` still races with any concurrent `env::var` on another thread,
/// which is why every caller is `#[serial]`. The durable fix is a `TokenService::with_secret`
/// constructor in `may_auth` that takes no environment at all — tracked as a follow-up.
fn init_jwt_secret() {
    INIT_JWT_SECRET.call_once(|| {
        #[allow(
            unsafe_code,
            reason = "no non-env constructor for TokenService yet; see above"
        )]
        unsafe {
            std::env::set_var("MAY_JWT_SECRET", "test_secret_key_for_users_tests");
        }
    });
}

#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub enum CreateOutcome {
    Success,
    Failure,
}

#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub struct MockUserRepository {
    pub valid_user: Option<User>,
    pub create_outcome: CreateOutcome,
    pub deactivate_error: Option<AuthError>,
}

impl MockUserRepository {
    #[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
    pub fn success(valid_user: Option<User>) -> Self {
        Self {
            valid_user,
            create_outcome: CreateOutcome::Success,
            deactivate_error: None,
        }
    }

    #[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
    pub fn with_missing_user() -> Self {
        Self {
            valid_user: None,
            create_outcome: CreateOutcome::Failure,
            deactivate_error: Some(AuthError::UserNotFound),
        }
    }
}

#[async_trait::async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<User, AuthError> {
        self.valid_user
            .as_ref()
            .filter(|u| u.username == username)
            .cloned()
            .ok_or(AuthError::UserNotFound)
    }

    async fn create(
        &self,
        username: &str,
        _password_hash: &str,
        role: Role,
    ) -> Result<User, AuthError> {
        match self.create_outcome {
            CreateOutcome::Success => Ok(User {
                id: uuid::Uuid::new_v4(),
                username: username.to_string(),
                password_hash: "[hashed]".to_string(),
                role,
                is_active: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }),
            CreateOutcome::Failure => Err(AuthError::InvalidCredentials),
        }
    }

    async fn list(&self, _page: u32, _per_page: u32) -> Result<Vec<User>, AuthError> {
        match &self.valid_user {
            Some(u) => Ok(vec![u.clone()]),
            None => Ok(vec![]),
        }
    }

    async fn deactivate(&self, _id: uuid::Uuid) -> Result<(), AuthError> {
        match &self.deactivate_error {
            None => Ok(()),
            Some(AuthError::UserNotFound) => Err(AuthError::UserNotFound),
            Some(_) => Err(AuthError::InvalidCredentials),
        }
    }

    async fn update(
        &self,
        _id: uuid::Uuid,
        _role: Option<Role>,
        _password_hash: Option<String>,
    ) -> Result<User, AuthError> {
        Err(AuthError::UserNotFound)
    }
}

#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub fn test_state() -> AppState {
    test_state_with_repo(MockUserRepository::success(None))
}

#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub fn test_state_with_repo(mock_repo: MockUserRepository) -> AppState {
    init_jwt_secret();
    let token_service = Arc::new(TokenService::new().expect("token service initialises"));
    let user_repository = Arc::new(mock_repo);

    AppState {
        user_repository,
        token_service,
        state_mgr: Arc::new(may_core::StateMgr::new()),
        dialect_kind: "postgres".into(),
    }
}

#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub fn test_app() -> axum::Router {
    may_rest::build_router(test_state())
}

#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub fn test_app_with_repo(mock_repo: MockUserRepository) -> axum::Router {
    may_rest::build_router(test_state_with_repo(mock_repo))
}

/// Mint a JWT with the given role claim using the shared test secret.
#[allow(dead_code, reason = "shared fixture used by a subset of test modules")]
pub fn mint_token(role: &str) -> String {
    init_jwt_secret();
    let user = User {
        id: uuid::Uuid::new_v4(),
        username: "token_owner".to_string(),
        password_hash: String::new(),
        role: match role {
            "admin" => Role::Admin,
            _ => Role::Viewer,
        },
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let ts = TokenService::new().expect("token service initialises");
    ts.issue(&user).expect("token issues")
}
