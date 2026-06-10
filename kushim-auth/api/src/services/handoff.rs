use redis::AsyncCommands;
use thiserror::Error;
use uuid::Uuid;

const HANDOFF_TTL_SECONDS: u64 = 60;
const HANDOFF_KEY_PREFIX: &str = "handoff:";

#[derive(Debug, Error)]
pub enum HandoffError {
    #[error("redis unavailable")]
    RedisUnavailable,
    #[error("invalid or expired handoff code")]
    InvalidCode,
}

#[derive(Clone)]
pub struct HandoffService {
    client: redis::Client,
}

impl HandoffService {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { client })
    }

    pub async fn create_code(
        &self,
        access_token: &str,
        refresh_token: &str,
    ) -> Result<String, HandoffError> {
        let code = generate_code();
        let key = format!("{HANDOFF_KEY_PREFIX}{code}");
        let value = serde_json::json!({
            "at": access_token,
            "rt": refresh_token,
        })
        .to_string();

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| HandoffError::RedisUnavailable)?;

        conn.set_ex::<_, _, ()>(&key, &value, HANDOFF_TTL_SECONDS)
            .await
            .map_err(|_| HandoffError::RedisUnavailable)?;

        Ok(code)
    }

    pub async fn exchange_code(&self, code: &str) -> Result<(String, String), HandoffError> {
        if code.is_empty() || code.len() > 64 {
            return Err(HandoffError::InvalidCode);
        }

        let key = format!("{HANDOFF_KEY_PREFIX}{code}");

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| HandoffError::RedisUnavailable)?;

        let value: Option<String> = redis::cmd("GETDEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|_| HandoffError::RedisUnavailable)?;

        let value = value.ok_or(HandoffError::InvalidCode)?;
        let parsed: serde_json::Value =
            serde_json::from_str(&value).map_err(|_| HandoffError::InvalidCode)?;

        let access_token = parsed["at"]
            .as_str()
            .ok_or(HandoffError::InvalidCode)?
            .to_string();
        let refresh_token = parsed["rt"]
            .as_str()
            .ok_or(HandoffError::InvalidCode)?
            .to_string();

        Ok((access_token, refresh_token))
    }
}

fn generate_code() -> String {
    Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::generate_code;

    #[test]
    fn generated_code_is_32_hex_chars() {
        let code = generate_code();
        assert_eq!(code.len(), 32);
        assert!(code.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generated_codes_are_unique() {
        let a = generate_code();
        let b = generate_code();
        assert_ne!(a, b);
    }
}
