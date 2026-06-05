use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use thiserror::Error;

const MIN_RECOVERY_PHRASE_LENGTH: usize = 16;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoveryServiceError {
    #[error("recovery phrase must not be blank")]
    BlankRecoveryPhrase,
    #[error("recovery phrase must be at least {minimum} characters long")]
    TooShort { minimum: usize },
    #[error("stored recovery phrase hash is invalid")]
    InvalidStoredHash,
    #[error("failed to hash recovery phrase")]
    HashingFailed,
}

#[derive(Debug, Clone, Default)]
pub struct RecoveryService;

impl RecoveryService {
    pub fn new() -> Self {
        Self
    }

    pub fn hash_recovery_phrase(&self, plaintext: &str) -> Result<String, RecoveryServiceError> {
        self.validate_recovery_phrase(plaintext)?;

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let phrase_hash = argon2
            .hash_password(plaintext.as_bytes(), &salt)
            .map_err(|_| RecoveryServiceError::HashingFailed)?
            .to_string();

        Ok(phrase_hash)
    }

    pub fn verify_recovery_phrase(
        &self,
        plaintext: &str,
        phrase_hash: &str,
    ) -> Result<bool, RecoveryServiceError> {
        let parsed_hash =
            PasswordHash::new(phrase_hash).map_err(|_| RecoveryServiceError::InvalidStoredHash)?;
        let argon2 = Argon2::default();

        match argon2.verify_password(plaintext.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(_) => Err(RecoveryServiceError::InvalidStoredHash),
        }
    }

    pub fn validate_recovery_phrase(&self, plaintext: &str) -> Result<(), RecoveryServiceError> {
        if plaintext.trim().is_empty() {
            return Err(RecoveryServiceError::BlankRecoveryPhrase);
        }

        let length = plaintext.chars().count();
        if length < MIN_RECOVERY_PHRASE_LENGTH {
            return Err(RecoveryServiceError::TooShort {
                minimum: MIN_RECOVERY_PHRASE_LENGTH,
            });
        }

        Ok(())
    }
}
