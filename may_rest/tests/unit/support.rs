use may_auth::{
    error::AuthError,
    models::{Role, User},
    repository::UserRepository,
    token::TokenService,
};
use may_rest::AppState;
use std::sync::Arc;

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
    #[allow(unsafe_code, reason = "set_var required to set secret for test app")]
    unsafe {
        std::env::set_var("MAY_JWT_SECRET", "test_secret_key_for_users_tests");
    }
    let token_service = Arc::new(TokenService::new().expect("token service initialises"));
    let user_repository = Arc::new(mock_repo);

    AppState {
        user_repository,
        token_service,
        state_mgr: Arc::new(may_core::StateMgr::new()),
        dialect_kind: "postgres".to_string(),
    }
}
