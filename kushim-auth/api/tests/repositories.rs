use kushim_auth_api::{
    db,
    repositories::{
        recovery_phrases::RecoveryPhraseRepository, revoked_tokens::RevokedTokenRepository,
        roles::RoleRepository, users::UserRepository,
    },
};
use std::sync::OnceLock;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
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
    let mut prefix = prefix
        .chars()
        .filter(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || *char == '_')
        .collect::<String>();

    if prefix.is_empty() {
        prefix = "test".to_string();
    }

    let max_prefix_len = 40usize.saturating_sub(1 + short_uuid.len());
    if prefix.len() > max_prefix_len {
        prefix.truncate(max_prefix_len);
    }

    format!("{prefix}_{short_uuid}")
}

#[tokio::test]
async fn role_repository_finds_user_role() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let repository = RoleRepository::new(pool);

    let role = repository
        .find_user_role()
        .await
        .expect("find user role")
        .expect("user role exists");

    assert_eq!(role.label, "user");
}

#[tokio::test]
async fn user_repository_creates_and_reads_user() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let roles = RoleRepository::new(pool.clone());
    let users = UserRepository::new(pool.clone());
    let role = roles
        .find_user_role()
        .await
        .expect("query role")
        .expect("user role exists");
    let username = unique_handle("repo_user");

    let created = users
        .create_user(role.id_role, &username, &username, "$argon2id$placeholder")
        .await
        .expect("create user");

    let by_id = users
        .find_by_id(created.id_user)
        .await
        .expect("find by id")
        .expect("user exists");
    let by_username = users
        .find_active_by_username(&username)
        .await
        .expect("find by username")
        .expect("active user exists");

    assert_eq!(by_id.id_user, created.id_user);
    assert_eq!(by_username.username, username);
}

#[tokio::test]
async fn recovery_phrase_repository_upserts_phrase() {
    let pool = test_pool().await;
    ensure_user_role(&pool).await;
    let roles = RoleRepository::new(pool.clone());
    let users = UserRepository::new(pool.clone());
    let recovery_repository = RecoveryPhraseRepository::new(pool.clone());
    let role = roles
        .find_user_role()
        .await
        .expect("query role")
        .expect("user role exists");
    let username = unique_handle("repo_recovery");

    let user = users
        .create_user(role.id_role, &username, &username, "$argon2id$placeholder")
        .await
        .expect("create user");

    let first = recovery_repository
        .upsert_for_user(user.id_user, "$argon2id$phrase1")
        .await
        .expect("upsert first phrase");
    let second = recovery_repository
        .upsert_for_user(user.id_user, "$argon2id$phrase2")
        .await
        .expect("upsert second phrase");
    let stored = recovery_repository
        .find_by_user_id(user.id_user)
        .await
        .expect("find phrase")
        .expect("phrase exists");

    assert_eq!(first.id_user, user.id_user);
    assert_eq!(second.id_user, user.id_user);
    assert_eq!(stored.phrase_hash, "$argon2id$phrase2");
}

#[tokio::test]
async fn revoked_token_repository_revokes_and_detects_token() {
    let pool = test_pool().await;
    let repository = RevokedTokenRepository::new(pool);
    let jti = format!("test-jti-{}", Uuid::new_v4());
    let expires_at = OffsetDateTime::now_utc() + Duration::hours(1);

    let revoked = repository
        .revoke_token(&jti, "refresh", expires_at, None)
        .await
        .expect("revoke token");
    let exists = repository
        .is_revoked(&jti)
        .await
        .expect("check revoked token");

    assert_eq!(revoked.jti, jti);
    assert!(exists);
}
