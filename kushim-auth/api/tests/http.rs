use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kushim_auth_api::{
    db, http,
    repositories::{
        recovery_phrases::RecoveryPhraseRepository, revoked_tokens::RevokedTokenRepository,
        roles::RoleRepository, users::UserRepository,
    },
    services::{
        auth::AuthService, password::PasswordService, recovery::RecoveryService,
        token::TokenService,
    },
    state::AppState,
};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tower::util::ServiceExt;
use uuid::Uuid;

static ROLE_FIXTURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
        SELECT COALESCE(MAX(id_role), 0) + 1
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

fn unique_handle(prefix: &str) -> String {
    let short_uuid = Uuid::new_v4().simple().to_string();
    let short_uuid = &short_uuid[..12];
    format!("{prefix}_{short_uuid}")
}

fn build_app_state(pool: sqlx::PgPool) -> AppState {
    let auth_service = AuthService::new(
        RoleRepository::new(pool.clone()),
        UserRepository::new(pool.clone()),
        RecoveryPhraseRepository::new(pool.clone()),
        RevokedTokenRepository::new(pool.clone()),
        PasswordService::new(),
        RecoveryService::new(),
        TokenService::new(
            "dev_only_change_me_minimum_32_chars",
            "kushim-auth".to_string(),
            900,
            2_592_000,
        ),
    );

    AppState {
        db_pool: pool,
        auth_service,
        rate_limiter: None,
        rate_limit_enabled: false,
        service_name: "kushim-auth",
        service_version: env!("CARGO_PKG_VERSION"),
        routes_version: "auth-routes-v1",
        environment: "test".to_string(),
    }
}

async fn json_response(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("parse json response")
}

#[tokio::test]
async fn invalid_login_returns_generic_401() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let app = http::router(build_app_state(pool.clone()));
    let public_handle = unique_handle("httplogin");

    let signup_request = Request::builder()
        .method("POST")
        .uri("/auth/signup")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "username": "HTTP Login User",
                "public_handle": public_handle,
                "password": "correct horse battery"
            })
            .to_string(),
        ))
        .expect("build signup request");

    let signup_response = app
        .clone()
        .oneshot(signup_request)
        .await
        .expect("signup response");
    assert_eq!(signup_response.status(), StatusCode::CREATED);

    let login_request = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "public_handle": public_handle,
                "password": "wrong password"
            })
            .to_string(),
        ))
        .expect("build login request");

    let response = app.oneshot(login_request).await.expect("login response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = json_response(response).await;
    assert_eq!(body["error"]["code"], "invalid_credentials");
    assert_eq!(body["error"]["message"], "invalid credentials");
}

#[tokio::test]
async fn duplicate_signup_returns_409() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let app = http::router(build_app_state(pool));
    let public_handle = unique_handle("httpdup");

    let payload = json!({
        "username": "Duplicate User",
        "public_handle": public_handle,
        "password": "correct horse battery"
    })
    .to_string();

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(payload.clone()))
                .expect("build first signup request"),
        )
        .await
        .expect("first signup response");
    assert_eq!(first_response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .expect("build second signup request"),
        )
        .await
        .expect("second signup response");

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = json_response(response).await;
    assert_eq!(body["error"]["code"], "public_handle_conflict");
}

#[tokio::test]
async fn missing_bearer_token_returns_401() {
    let pool = test_pool().await;
    let app = http::router(build_app_state(pool));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/me")
                .body(Body::empty())
                .expect("build me request"),
        )
        .await
        .expect("me response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = json_response(response).await;
    assert_eq!(body["error"]["code"], "missing_bearer_token");
}

#[tokio::test]
async fn invalid_recovery_phrase_returns_401() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let app = http::router(build_app_state(pool));
    let public_handle = unique_handle("httprecovery");

    let signup_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "HTTP Recovery User",
                        "public_handle": public_handle,
                        "password": "correct horse battery"
                    })
                    .to_string(),
                ))
                .expect("build signup request"),
        )
        .await
        .expect("signup response");
    assert_eq!(signup_response.status(), StatusCode::CREATED);
    let signup_body = json_response(signup_response).await;
    let access_token = signup_body["access_token"]
        .as_str()
        .expect("access token")
        .to_string();

    let setup_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/recovery/setup")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {access_token}"))
                .body(Body::from(
                    json!({
                        "current_password": "correct horse battery",
                        "recovery_phrase": "this is a long recovery phrase"
                    })
                    .to_string(),
                ))
                .expect("build recovery setup request"),
        )
        .await
        .expect("setup response");
    assert_eq!(setup_response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/recovery/reset-password")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "public_handle": public_handle,
                        "recovery_phrase": "this is the wrong recovery phrase",
                        "new_password": "a brand new secure password"
                    })
                    .to_string(),
                ))
                .expect("build reset request"),
        )
        .await
        .expect("reset response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = json_response(response).await;
    assert_eq!(body["error"]["code"], "invalid_recovery_phrase");
}

#[tokio::test]
async fn validation_failure_returns_400() {
    let pool = test_pool().await;
    let app = http::router(build_app_state(pool));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": " ",
                        "public_handle": "invalid handle",
                        "password": "short"
                    })
                    .to_string(),
                ))
                .expect("build invalid signup request"),
        )
        .await
        .expect("validation response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_response(response).await;
    assert!(body["error"]["code"].is_string());
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn login_response_includes_no_store_headers() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let app = http::router(build_app_state(pool));
    let public_handle = unique_handle("httpheaders");

    let signup_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "Headers User",
                        "public_handle": public_handle,
                        "password": "correct horse battery"
                    })
                    .to_string(),
                ))
                .expect("build signup request"),
        )
        .await
        .expect("signup response");

    assert_eq!(signup_response.status(), StatusCode::CREATED);
    assert_eq!(
        signup_response
            .headers()
            .get("cache-control")
            .expect("cache-control"),
        "no-store"
    );
    assert_eq!(
        signup_response.headers().get("pragma").expect("pragma"),
        "no-cache"
    );
    assert_eq!(
        signup_response
            .headers()
            .get("x-content-type-options")
            .expect("nosniff"),
        "nosniff"
    );
}

#[tokio::test]
async fn auth_error_response_includes_no_store_headers() {
    let pool = test_pool().await;
    let app = http::router(build_app_state(pool));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/me")
                .body(Body::empty())
                .expect("build me request"),
        )
        .await
        .expect("me response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .expect("cache-control"),
        "no-store"
    );
}

#[tokio::test]
async fn login_rejects_unknown_json_fields() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let app = http::router(build_app_state(pool));
    let public_handle = unique_handle("httpunknownlogin");

    let signup_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "Unknown Field Login User",
                        "public_handle": public_handle,
                        "password": "correct horse battery"
                    })
                    .to_string(),
                ))
                .expect("build signup request"),
        )
        .await
        .expect("signup response");
    assert_eq!(signup_response.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "public_handle": "placeholder",
                        "password": "correct horse battery",
                        "extra_field": "unexpected"
                    })
                    .to_string(),
                ))
                .expect("build login request"),
        )
        .await
        .expect("login response");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn signup_rejects_unknown_json_fields() {
    let pool = test_pool().await;
    let app = http::router(build_app_state(pool));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "Unknown Signup User",
                        "public_handle": unique_handle("httpunknownsignup"),
                        "password": "correct horse battery",
                        "extra_field": "unexpected"
                    })
                    .to_string(),
                ))
                .expect("build signup request"),
        )
        .await
        .expect("signup response");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn refresh_rejects_unknown_json_fields() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let app = http::router(build_app_state(pool));
    let public_handle = unique_handle("httpunknownrefresh");

    let signup_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "Unknown Refresh User",
                        "public_handle": public_handle,
                        "password": "correct horse battery"
                    })
                    .to_string(),
                ))
                .expect("build signup request"),
        )
        .await
        .expect("signup response");
    assert_eq!(signup_response.status(), StatusCode::CREATED);
    let signup_body = json_response(signup_response).await;
    let refresh_token = signup_body["refresh_token"]
        .as_str()
        .expect("refresh token")
        .to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "refresh_token": refresh_token,
                        "extra_field": "unexpected"
                    })
                    .to_string(),
                ))
                .expect("build refresh request"),
        )
        .await
        .expect("refresh response");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
