use kushim_auth_api::{
    db,
    http::auth::{
        LoginRequest, RecoverySetupRequest, RefreshRequest, ResetPasswordRequest, SignupRequest,
    },
    repositories::{
        recovery_phrases::RecoveryPhraseRepository, revoked_tokens::RevokedTokenRepository,
        roles::RoleRepository, users::UserRepository,
    },
    services::{
        auth::{AuthService, AuthServiceError},
        password::PasswordService,
        recovery::RecoveryService,
        token::TokenService,
    },
};
use std::sync::OnceLock;
use tokio::sync::Mutex;
use uuid::Uuid;

static ROLE_FIXTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn unique_username(prefix: &str) -> String {
    let short = &Uuid::new_v4().simple().to_string()[..8];
    format!("{prefix}_{short}")
}

async fn test_pool() -> sqlx::PgPool {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://kushim:kushim_secret_dev@localhost:5432/kushim".to_string()
    });

    db::create_pool(&database_url)
        .await
        .expect("create test pool")
}

async fn ensure_user_role(pool: &sqlx::PgPool) {
    let lock = ROLE_FIXTURE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;

    let existing_role_id = sqlx::query_scalar::<_, i16>(
        r#"
        SELECT id_role
        FROM roles
        WHERE label = 'user'
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .expect("select user role fixture");

    if existing_role_id.is_some() {
        return;
    }

    let next_role_id = sqlx::query_scalar::<_, i16>(
        r#"
        SELECT (COALESCE(MAX(id_role), 0) + 1)::smallint
        FROM roles
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("compute next role fixture id");

    sqlx::query(
        r#"
        INSERT INTO roles (id_role, label)
        VALUES ($1, 'user')
        "#,
    )
    .bind(next_role_id)
    .execute(pool)
    .await
    .expect("insert user role fixture");
}

fn build_auth_service(pool: sqlx::PgPool) -> AuthService {
    let roles = RoleRepository::new(pool.clone());
    let users = UserRepository::new(pool.clone());
    let recovery_phrases = RecoveryPhraseRepository::new(pool.clone());
    let revoked_tokens = RevokedTokenRepository::new(pool);
    let password_service = PasswordService::new();
    let recovery_service = RecoveryService::new();
    let token_service = TokenService::new(
        "dev_only_change_me_minimum_32_chars",
        "kushim-auth".to_string(),
        900,
        2_592_000,
    );

    AuthService::new(
        roles,
        users,
        recovery_phrases,
        revoked_tokens,
        password_service,
        recovery_service,
        token_service,
    )
}

#[tokio::test]
async fn auth_signup_creates_user_and_returns_tokens() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("signup");

    let response = service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    assert_eq!(response.user.public_handle, uname);
    assert!(!response.access_token.is_empty());
    assert!(!response.refresh_token.is_empty());
}

#[tokio::test]
async fn auth_signup_rejects_duplicate_username() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("dup");

    service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("first signup should succeed");

    let error = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect_err("second signup should fail");

    assert_eq!(error, AuthServiceError::UsernameAlreadyExists);
}

#[tokio::test]
async fn auth_login_works_with_correct_password() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("login_ok");

    service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let response = service
        .login(LoginRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("login should succeed");

    assert!(!response.access_token.is_empty());
    assert!(!response.refresh_token.is_empty());
}

#[tokio::test]
async fn auth_login_fails_with_wrong_password() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("login_bad");

    service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let error = service
        .login(LoginRequest {
            username: uname,
            password: "wrong password value".to_string(),
        })
        .await
        .expect_err("login should fail");

    assert_eq!(error, AuthServiceError::InvalidCredentials);
}

#[tokio::test]
async fn auth_refresh_rotates_refresh_token_and_old_one_cannot_be_reused() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("refresh");

    let signup = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let rotated = service
        .refresh(RefreshRequest {
            refresh_token: signup.refresh_token.clone(),
        })
        .await
        .expect("refresh should succeed");

    assert_ne!(rotated.refresh_token, signup.refresh_token);

    let error = service
        .refresh(RefreshRequest {
            refresh_token: signup.refresh_token,
        })
        .await
        .expect_err("old refresh token should be rejected");

    assert_eq!(error, AuthServiceError::RefreshTokenRevoked);
}

#[tokio::test]
async fn auth_logout_revokes_refresh_token() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("logout");

    let signup = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let logout = service
        .logout(kushim_auth_api::http::auth::LogoutRequest {
            refresh_token: signup.refresh_token.clone(),
        })
        .await
        .expect("logout should succeed");

    assert!(logout.success);

    let second_logout = service
        .logout(kushim_auth_api::http::auth::LogoutRequest {
            refresh_token: signup.refresh_token.clone(),
        })
        .await
        .expect("second logout should remain idempotent for revoked refresh token");

    assert!(second_logout.success);

    let error = service
        .refresh(RefreshRequest {
            refresh_token: signup.refresh_token,
        })
        .await
        .expect_err("revoked refresh token should fail");

    assert_eq!(error, AuthServiceError::RefreshTokenRevoked);
}

#[tokio::test]
async fn auth_logout_rejects_access_token() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("logout_at");

    let signup = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let error = service
        .logout(kushim_auth_api::http::auth::LogoutRequest {
            refresh_token: signup.access_token,
        })
        .await
        .expect_err("access token should be rejected by logout");

    assert_eq!(error, AuthServiceError::InvalidTokenType);
}

#[tokio::test]
async fn auth_logout_rejects_malformed_token() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);

    let error = service
        .logout(kushim_auth_api::http::auth::LogoutRequest {
            refresh_token: "not-a-jwt".to_string(),
        })
        .await
        .expect_err("malformed token should be rejected by logout");

    assert_eq!(error, AuthServiceError::InvalidToken);
}

#[tokio::test]
async fn auth_me_returns_user_with_valid_access_token() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("me");

    let signup = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let me = service
        .me(&signup.access_token)
        .await
        .expect("me should succeed");

    assert_eq!(me.user.public_handle, signup.user.public_handle);
}

#[tokio::test]
async fn auth_me_rejects_invalid_access_token() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);

    let error = service
        .me("not-a-valid-jwt")
        .await
        .expect_err("invalid access token should fail");

    assert_eq!(error, AuthServiceError::InvalidToken);
}

#[tokio::test]
async fn recovery_setup_recovery_phrase() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool.clone());
    let uname = unique_username("rec_setup");

    let signup = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let response = service
        .setup_recovery_phrase(
            &signup.access_token,
            RecoverySetupRequest {
                current_password: "correct horse battery".to_string(),
                recovery_phrase: "this is a long recovery phrase".to_string(),
            },
        )
        .await
        .expect("setup recovery should succeed");

    assert!(response.success);

    let recovery_setup_completed = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT recovery_setup_completed
        FROM users
        WHERE public_handle = $1
        "#,
    )
    .bind(signup.user.public_handle)
    .fetch_one(&pool)
    .await
    .expect("load recovery flag");

    assert!(recovery_setup_completed);
}

#[tokio::test]
async fn recovery_setup_requires_valid_access_token() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);

    let error = service
        .setup_recovery_phrase(
            "not-a-valid-jwt",
            RecoverySetupRequest {
                current_password: "correct horse battery".to_string(),
                recovery_phrase: "this is a long recovery phrase".to_string(),
            },
        )
        .await
        .expect_err("invalid access token should fail");

    assert_eq!(error, AuthServiceError::InvalidToken);
}

#[tokio::test]
async fn recovery_setup_fails_with_wrong_current_password() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("rec_wpw");

    let signup = service
        .signup(SignupRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let error = service
        .setup_recovery_phrase(
            &signup.access_token,
            RecoverySetupRequest {
                current_password: "wrong current password".to_string(),
                recovery_phrase: "this is a long recovery phrase".to_string(),
            },
        )
        .await
        .expect_err("wrong current password should fail");

    assert_eq!(error, AuthServiceError::InvalidCredentials);
}

#[tokio::test]
async fn recovery_reset_password_with_valid_phrase_and_rotation() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("reset_pw");

    let signup = service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    service
        .setup_recovery_phrase(
            &signup.access_token,
            RecoverySetupRequest {
                current_password: "correct horse battery".to_string(),
                recovery_phrase: "this is a long recovery phrase".to_string(),
            },
        )
        .await
        .expect("recovery setup should succeed");

    let response = service
        .reset_password(ResetPasswordRequest {
            username: uname.clone(),
            recovery_phrase: "this is a long recovery phrase".to_string(),
            new_password: "a brand new secure password".to_string(),
            new_recovery_phrase: "brand new recovery phrase words here".to_string(),
        })
        .await
        .expect("reset password should succeed");

    assert!(response.success);

    service
        .login(LoginRequest {
            username: uname.clone(),
            password: "a brand new secure password".to_string(),
        })
        .await
        .expect("login with new password should succeed");

    let error = service
        .login(LoginRequest {
            username: uname,
            password: "correct horse battery".to_string(),
        })
        .await
        .expect_err("old password should fail");

    assert_eq!(error, AuthServiceError::InvalidCredentials);
}

#[tokio::test]
async fn recovery_reset_old_phrase_no_longer_works_after_rotation() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("rot_chk");

    let signup = service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    service
        .setup_recovery_phrase(
            &signup.access_token,
            RecoverySetupRequest {
                current_password: "correct horse battery".to_string(),
                recovery_phrase: "original recovery phrase for this user".to_string(),
            },
        )
        .await
        .expect("recovery setup should succeed");

    service
        .reset_password(ResetPasswordRequest {
            username: uname.clone(),
            recovery_phrase: "original recovery phrase for this user".to_string(),
            new_password: "a brand new secure password".to_string(),
            new_recovery_phrase: "rotated recovery phrase for this user".to_string(),
        })
        .await
        .expect("reset password should succeed");

    let error = service
        .reset_password(ResetPasswordRequest {
            username: uname,
            recovery_phrase: "original recovery phrase for this user".to_string(),
            new_password: "yet another secure password".to_string(),
            new_recovery_phrase: "another rotated phrase for user".to_string(),
        })
        .await
        .expect_err("old recovery phrase should no longer work");

    assert_eq!(error, AuthServiceError::InvalidRecoveryPhrase);
}

#[tokio::test]
async fn recovery_reset_new_phrase_works_after_rotation() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("new_phr");

    let signup = service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    service
        .setup_recovery_phrase(
            &signup.access_token,
            RecoverySetupRequest {
                current_password: "correct horse battery".to_string(),
                recovery_phrase: "first recovery phrase for the user".to_string(),
            },
        )
        .await
        .expect("recovery setup should succeed");

    service
        .reset_password(ResetPasswordRequest {
            username: uname.clone(),
            recovery_phrase: "first recovery phrase for the user".to_string(),
            new_password: "second secure password here".to_string(),
            new_recovery_phrase: "second recovery phrase for the user".to_string(),
        })
        .await
        .expect("first reset should succeed");

    service
        .reset_password(ResetPasswordRequest {
            username: uname.clone(),
            recovery_phrase: "second recovery phrase for the user".to_string(),
            new_password: "third secure password here".to_string(),
            new_recovery_phrase: "third recovery phrase for the user".to_string(),
        })
        .await
        .expect("second reset with new phrase should succeed");

    service
        .login(LoginRequest {
            username: uname,
            password: "third secure password here".to_string(),
        })
        .await
        .expect("login with latest password should succeed");
}

#[tokio::test]
async fn recovery_reset_fails_with_wrong_phrase() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("rst_wph");

    let signup = service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    service
        .setup_recovery_phrase(
            &signup.access_token,
            RecoverySetupRequest {
                current_password: "correct horse battery".to_string(),
                recovery_phrase: "this is a long recovery phrase".to_string(),
            },
        )
        .await
        .expect("recovery setup should succeed");

    let error = service
        .reset_password(ResetPasswordRequest {
            username: uname,
            recovery_phrase: "this is the wrong recovery phrase".to_string(),
            new_password: "a brand new secure password".to_string(),
            new_recovery_phrase: "brand new recovery phrase words".to_string(),
        })
        .await
        .expect_err("wrong recovery phrase should fail");

    assert_eq!(error, AuthServiceError::InvalidRecoveryPhrase);
}

#[tokio::test]
async fn recovery_reset_fails_if_no_recovery_phrase_configured() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let service = build_auth_service(pool);
    let uname = unique_username("rst_nop");

    service
        .signup(SignupRequest {
            username: uname.clone(),
            password: "correct horse battery".to_string(),
        })
        .await
        .expect("signup should succeed");

    let error = service
        .reset_password(ResetPasswordRequest {
            username: uname,
            recovery_phrase: "this is a long recovery phrase".to_string(),
            new_password: "a brand new secure password".to_string(),
            new_recovery_phrase: "brand new recovery phrase words".to_string(),
        })
        .await
        .expect_err("missing recovery phrase should fail");

    assert_eq!(error, AuthServiceError::RecoveryPhraseNotConfigured);
}
