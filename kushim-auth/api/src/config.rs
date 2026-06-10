use anyhow::{Context, Result, bail};
use std::{env, net::SocketAddr};

const DEV_JWT_SECRET: &str = "dev_only_change_me_minimum_32_chars";

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub redis_url: Option<String>,
    pub rate_limit_enabled: bool,
    pub host: String,
    pub port: u16,
    pub rust_log: String,
    pub environment: String,
    pub auth_jwt_secret: String,
    pub jwt_issuer: String,
    pub access_token_ttl_seconds: i64,
    pub refresh_token_ttl_seconds: i64,
    pub cors_allowed_origin: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").context("DATABASE_URL must be set for kushim-auth-api")?;
        let redis_url = env::var("REDIS_URL").ok();
        let rate_limit_enabled = env::var("RATE_LIMIT_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .context("RATE_LIMIT_ENABLED must be a valid boolean")?;
        let host = env::var("AUTH_SERVICE_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("AUTH_SERVICE_PORT")
            .unwrap_or_else(|_| "3002".to_string())
            .parse::<u16>()
            .context("AUTH_SERVICE_PORT must be a valid u16")?;
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let environment = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let auth_jwt_secret = env::var("AUTH_JWT_SECRET")
            .context("AUTH_JWT_SECRET must be set for kushim-auth-api")?;
        let jwt_issuer = env::var("JWT_ISSUER").unwrap_or_else(|_| "kushim-auth".to_string());
        let access_token_ttl_seconds = env::var("ACCESS_TOKEN_TTL_SECONDS")
            .unwrap_or_else(|_| "900".to_string())
            .parse::<i64>()
            .context("ACCESS_TOKEN_TTL_SECONDS must be a valid i64")?;
        let refresh_token_ttl_seconds = env::var("REFRESH_TOKEN_TTL_SECONDS")
            .unwrap_or_else(|_| "2592000".to_string())
            .parse::<i64>()
            .context("REFRESH_TOKEN_TTL_SECONDS must be a valid i64")?;

        let cors_allowed_origin = env::var("CORS_ALLOWED_ORIGIN").ok().and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

        if host.trim().is_empty() {
            bail!("AUTH_SERVICE_HOST must not be blank");
        }

        if rate_limit_enabled {
            let redis_url = redis_url
                .as_ref()
                .context("REDIS_URL must be set when RATE_LIMIT_ENABLED=true")?;
            if redis_url.trim().is_empty() {
                bail!("REDIS_URL must not be blank when RATE_LIMIT_ENABLED=true");
            }
        }

        if auth_jwt_secret.len() < 32 {
            bail!("AUTH_JWT_SECRET must be at least 32 characters long");
        }

        validate_jwt_secret_for_environment(&environment, &auth_jwt_secret)?;

        if jwt_issuer.trim().is_empty() {
            bail!("JWT_ISSUER must not be blank");
        }

        if access_token_ttl_seconds <= 0 {
            bail!("ACCESS_TOKEN_TTL_SECONDS must be greater than 0");
        }

        if refresh_token_ttl_seconds <= 0 {
            bail!("REFRESH_TOKEN_TTL_SECONDS must be greater than 0");
        }

        Ok(Self {
            database_url,
            redis_url,
            rate_limit_enabled,
            host,
            port,
            rust_log,
            environment,
            auth_jwt_secret,
            jwt_issuer,
            access_token_ttl_seconds,
            refresh_token_ttl_seconds,
            cors_allowed_origin,
        })
    }

    pub fn socket_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .context("AUTH_SERVICE_HOST and AUTH_SERVICE_PORT must produce a valid socket address")
    }
}

fn validate_jwt_secret_for_environment(environment: &str, secret: &str) -> Result<()> {
    if !environment.eq_ignore_ascii_case("production") {
        return Ok(());
    }

    if secret == DEV_JWT_SECRET {
        bail!("AUTH_JWT_SECRET must not use the documented development secret in production");
    }

    let lowered_secret = secret.to_ascii_lowercase();
    for placeholder in ["dev_only", "change_me", "changeme", "secret", "example"] {
        if lowered_secret.contains(placeholder) {
            bail!("AUTH_JWT_SECRET contains an obvious placeholder and is unsafe for production");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_jwt_secret_for_environment;

    #[test]
    fn production_rejects_documented_dev_secret() {
        let error = validate_jwt_secret_for_environment(
            "production",
            "dev_only_change_me_minimum_32_chars",
        )
        .expect_err("dev secret should be rejected in production");

        assert!(error.to_string().contains("documented development secret"));
    }

    #[test]
    fn production_accepts_safe_secret() {
        validate_jwt_secret_for_environment(
            "production",
            "A_Truly_Safe_Production_JWT_Key_123456789",
        )
        .expect("safe production secret should pass");
    }

    #[test]
    fn development_accepts_dev_secret() {
        validate_jwt_secret_for_environment("development", "dev_only_change_me_minimum_32_chars")
            .expect("development may use the documented dev secret");
    }
}
