use crate::errors::ApiError;
use redis::{AsyncCommands, Client, Script};
use std::time::{SystemTime, UNIX_EPOCH};

const LOGIN_IP_LIMIT: u64 = 20;
const LOGIN_HANDLE_LIMIT: u64 = 10;
const LOGIN_WINDOW_SECONDS: u64 = 600;
const SIGNUP_IP_LIMIT: u64 = 10;
const SIGNUP_WINDOW_SECONDS: u64 = 3600;
const RECOVERY_RESET_IP_LIMIT: u64 = 10;
const RECOVERY_RESET_HANDLE_LIMIT: u64 = 5;
const RECOVERY_RESET_WINDOW_SECONDS: u64 = 3600;
const RECOVERY_SETUP_USER_LIMIT: u64 = 5;
const RECOVERY_SETUP_IP_LIMIT: u64 = 20;
const RECOVERY_SETUP_WINDOW_SECONDS: u64 = 3600;
const REFRESH_IP_LIMIT: u64 = 60;
const REFRESH_WINDOW_SECONDS: u64 = 600;
const AUTH_GLOBAL_IP_LIMIT: u64 = 120;
const AUTH_GLOBAL_WINDOW_SECONDS: u64 = 60;

const FIXED_WINDOW_SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if current == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
local ttl = redis.call('TTL', KEYS[1])
return {current, ttl}
"#;

#[derive(Debug, Clone)]
pub struct RateLimitService {
    client: Client,
}

#[derive(Debug, Clone, Copy)]
pub struct LimitRule {
    pub scope: &'static str,
    pub max_attempts: u64,
    pub window_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitKey {
    pub scope: &'static str,
    pub identifier: String,
    pub window_bucket: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub current_count: u64,
    pub retry_after_seconds: u64,
}

impl RateLimitService {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = Client::open(redis_url)?;
        Ok(Self { client })
    }

    pub async fn check_health(&self) -> Result<(), redis::RedisError> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let pong: String = redis::cmd("PING").query_async(&mut connection).await?;
        if pong == "PONG" {
            Ok(())
        } else {
            Err(redis::RedisError::from((
                redis::ErrorKind::ResponseError,
                "unexpected Redis ping response",
            )))
        }
    }

    pub async fn enforce(&self, rule: LimitRule, identifier: &str) -> Result<(), ApiError> {
        let decision = self.check_rule(rule, identifier).await.map_err(|error| {
            tracing::error!(error = %error, scope = rule.scope, "rate limit check failed");
            ApiError::ServiceUnavailable
        })?;

        if decision.allowed {
            return Ok(());
        }

        tracing::warn!(
            event = "rate_limited",
            scope = rule.scope,
            identifier = identifier,
            retry_after_seconds = decision.retry_after_seconds,
            "rate limit triggered"
        );

        Err(ApiError::RateLimited {
            retry_after_seconds: decision.retry_after_seconds,
        })
    }

    pub async fn check_rule(
        &self,
        rule: LimitRule,
        identifier: &str,
    ) -> Result<RateLimitDecision, redis::RedisError> {
        let key = build_rate_limit_key(rule.scope, identifier, rule.window_seconds);
        let key_value = key.as_redis_key();
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let script = Script::new(FIXED_WINDOW_SCRIPT);
        let (current_count, retry_after_seconds): (u64, i64) = script
            .key(key_value)
            .arg(rule.window_seconds)
            .invoke_async(&mut connection)
            .await?;
        let retry_after_seconds = retry_after_seconds.max(0) as u64;

        Ok(RateLimitDecision {
            allowed: current_count <= rule.max_attempts,
            current_count,
            retry_after_seconds,
        })
    }

    pub async fn clear_scope(
        &self,
        rule: LimitRule,
        identifier: &str,
    ) -> Result<(), redis::RedisError> {
        let key = build_rate_limit_key(rule.scope, identifier, rule.window_seconds).as_redis_key();
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let _: usize = connection.del(key).await?;
        Ok(())
    }
}

pub fn global_auth_ip_rule() -> LimitRule {
    LimitRule {
        scope: "auth",
        max_attempts: AUTH_GLOBAL_IP_LIMIT,
        window_seconds: AUTH_GLOBAL_WINDOW_SECONDS,
    }
}

pub fn login_ip_rule() -> LimitRule {
    LimitRule {
        scope: "login:ip",
        max_attempts: LOGIN_IP_LIMIT,
        window_seconds: LOGIN_WINDOW_SECONDS,
    }
}

pub fn login_handle_rule() -> LimitRule {
    LimitRule {
        scope: "login:handle",
        max_attempts: LOGIN_HANDLE_LIMIT,
        window_seconds: LOGIN_WINDOW_SECONDS,
    }
}

pub fn signup_ip_rule() -> LimitRule {
    LimitRule {
        scope: "signup:ip",
        max_attempts: SIGNUP_IP_LIMIT,
        window_seconds: SIGNUP_WINDOW_SECONDS,
    }
}

pub fn recovery_reset_ip_rule() -> LimitRule {
    LimitRule {
        scope: "recovery_reset:ip",
        max_attempts: RECOVERY_RESET_IP_LIMIT,
        window_seconds: RECOVERY_RESET_WINDOW_SECONDS,
    }
}

pub fn recovery_reset_handle_rule() -> LimitRule {
    LimitRule {
        scope: "recovery_reset:handle",
        max_attempts: RECOVERY_RESET_HANDLE_LIMIT,
        window_seconds: RECOVERY_RESET_WINDOW_SECONDS,
    }
}

pub fn recovery_setup_ip_rule() -> LimitRule {
    LimitRule {
        scope: "recovery_setup:ip",
        max_attempts: RECOVERY_SETUP_IP_LIMIT,
        window_seconds: RECOVERY_SETUP_WINDOW_SECONDS,
    }
}

pub fn recovery_setup_user_rule() -> LimitRule {
    LimitRule {
        scope: "recovery_setup:user",
        max_attempts: RECOVERY_SETUP_USER_LIMIT,
        window_seconds: RECOVERY_SETUP_WINDOW_SECONDS,
    }
}

pub fn refresh_ip_rule() -> LimitRule {
    LimitRule {
        scope: "refresh:ip",
        max_attempts: REFRESH_IP_LIMIT,
        window_seconds: REFRESH_WINDOW_SECONDS,
    }
}

pub fn build_rate_limit_key(
    scope: &'static str,
    identifier: &str,
    window_seconds: u64,
) -> RateLimitKey {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs();
    let window_bucket = now / window_seconds;

    RateLimitKey {
        scope,
        identifier: sanitize_identifier(identifier),
        window_bucket,
    }
}

fn sanitize_identifier(identifier: &str) -> String {
    identifier
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

impl RateLimitKey {
    pub fn as_redis_key(&self) -> String {
        format!(
            "rate_limit:{}:{}:{}",
            self.scope, self.identifier, self.window_bucket
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LimitRule, RateLimitService, build_rate_limit_key, global_auth_ip_rule, login_handle_rule,
    };

    #[test]
    fn key_generation_matches_expected_prefix() {
        let key = build_rate_limit_key("login:ip", "127.0.0.1", 600).as_redis_key();

        assert!(key.starts_with("rate_limit:login:ip:127.0.0.1:"));
    }

    #[test]
    fn key_generation_sanitizes_identifier() {
        let key = build_rate_limit_key("login:handle", "alice:handle@example", 600);

        assert_eq!(key.identifier, "alice_handle_example");
    }

    #[test]
    fn rules_are_stable() {
        let global = global_auth_ip_rule();
        let login_handle = login_handle_rule();

        assert_eq!(global.scope, "auth");
        assert_eq!(global.max_attempts, 120);
        assert_eq!(login_handle.scope, "login:handle");
        assert_eq!(login_handle.max_attempts, 10);
    }

    #[tokio::test]
    async fn check_rule_reports_over_limit() {
        let service = match RateLimitService::new("redis://127.0.0.1:6379/15") {
            Ok(service) => service,
            Err(_) => return,
        };

        if service.check_health().await.is_err() {
            return;
        }

        let rule = LimitRule {
            scope: "test:limit",
            max_attempts: 2,
            window_seconds: 60,
        };
        let identifier = "rate_limit_test";
        let _ = service.clear_scope(rule, identifier).await;

        let first = service
            .check_rule(rule, identifier)
            .await
            .expect("first limit check");
        let second = service
            .check_rule(rule, identifier)
            .await
            .expect("second limit check");
        let third = service
            .check_rule(rule, identifier)
            .await
            .expect("third limit check");

        assert!(first.allowed);
        assert!(second.allowed);
        assert!(!third.allowed);

        let _ = service.clear_scope(rule, identifier).await;
    }
}
