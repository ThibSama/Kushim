use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use thiserror::Error;

const MIN_PASSWORD_LENGTH: usize = 12;
const MAX_PASSWORD_LENGTH: usize = 128;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PasswordServiceError {
    #[error("password must not be blank")]
    BlankPassword,
    #[error("password must be at least {minimum} characters long")]
    TooShort { minimum: usize },
    #[error("password must be at most {maximum} characters long")]
    TooLong { maximum: usize },
    #[error("stored password hash is invalid")]
    InvalidStoredHash,
    #[error("failed to hash password")]
    HashingFailed,
}

#[derive(Debug, Clone, Default)]
pub struct PasswordService;

impl PasswordService {
    pub fn new() -> Self {
        Self
    }

    pub fn hash_password(&self, plaintext: &str) -> Result<String, PasswordServiceError> {
        self.validate_password_policy(plaintext)?;

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(plaintext.as_bytes(), &salt)
            .map_err(|_| PasswordServiceError::HashingFailed)?
            .to_string();

        Ok(password_hash)
    }

    pub fn verify_password(
        &self,
        plaintext: &str,
        password_hash: &str,
    ) -> Result<bool, PasswordServiceError> {
        let parsed_hash = PasswordHash::new(password_hash)
            .map_err(|_| PasswordServiceError::InvalidStoredHash)?;
        let argon2 = Argon2::default();

        match argon2.verify_password(plaintext.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(_) => Err(PasswordServiceError::InvalidStoredHash),
        }
    }

    pub fn validate_password_policy(&self, plaintext: &str) -> Result<(), PasswordServiceError> {
        if plaintext.trim().is_empty() {
            return Err(PasswordServiceError::BlankPassword);
        }

        let length = plaintext.chars().count();

        if length < MIN_PASSWORD_LENGTH {
            return Err(PasswordServiceError::TooShort {
                minimum: MIN_PASSWORD_LENGTH,
            });
        }

        if length > MAX_PASSWORD_LENGTH {
            return Err(PasswordServiceError::TooLong {
                maximum: MAX_PASSWORD_LENGTH,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PASSWORD_LENGTH, MIN_PASSWORD_LENGTH, PasswordService, PasswordServiceError};

    #[test]
    fn valid_password_can_be_hashed() {
        let service = PasswordService::new();
        let password = "correct horse battery";

        let hash = service.hash_password(password).expect("hash password");

        assert!(!hash.is_empty());
    }

    #[test]
    fn hash_is_not_equal_to_plaintext() {
        let service = PasswordService::new();
        let password = "correct horse battery";

        let hash = service.hash_password(password).expect("hash password");

        assert_ne!(hash, password);
    }

    #[test]
    fn valid_password_verifies_successfully() {
        let service = PasswordService::new();
        let password = "correct horse battery";
        let hash = service.hash_password(password).expect("hash password");

        let is_valid = service
            .verify_password(password, &hash)
            .expect("verify password");

        assert!(is_valid);
    }

    #[test]
    fn invalid_password_does_not_verify() {
        let service = PasswordService::new();
        let hash = service
            .hash_password("correct horse battery")
            .expect("hash password");

        let is_valid = service
            .verify_password("wrong password value", &hash)
            .expect("verify password");

        assert!(!is_valid);
    }

    #[test]
    fn blank_password_is_rejected() {
        let service = PasswordService::new();

        let error = service
            .validate_password_policy("   ")
            .expect_err("blank password must fail");

        assert_eq!(error, PasswordServiceError::BlankPassword);
    }

    #[test]
    fn too_short_password_is_rejected() {
        let service = PasswordService::new();
        let password = "a".repeat(MIN_PASSWORD_LENGTH - 1);

        let error = service
            .validate_password_policy(&password)
            .expect_err("too-short password must fail");

        assert_eq!(
            error,
            PasswordServiceError::TooShort {
                minimum: MIN_PASSWORD_LENGTH,
            }
        );
    }

    #[test]
    fn too_long_password_is_rejected() {
        let service = PasswordService::new();
        let password = "a".repeat(MAX_PASSWORD_LENGTH + 1);

        let error = service
            .validate_password_policy(&password)
            .expect_err("too-long password must fail");

        assert_eq!(
            error,
            PasswordServiceError::TooLong {
                maximum: MAX_PASSWORD_LENGTH,
            }
        );
    }

    #[test]
    fn malformed_hash_returns_clean_error() {
        let service = PasswordService::new();

        let error = service
            .verify_password("correct horse battery", "not-a-valid-phc-hash")
            .expect_err("malformed hash must fail cleanly");

        assert_eq!(error, PasswordServiceError::InvalidStoredHash);
    }
}
