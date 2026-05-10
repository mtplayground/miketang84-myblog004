use std::{error::Error, fmt};

use argon2::{
    Argon2,
    password_hash::{
        Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::OsRng,
    },
};

#[derive(Debug)]
pub enum PasswordError {
    Hash(PasswordHashError),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hash(_) => write!(f, "password hashing operation failed"),
        }
    }
}

impl Error for PasswordError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<PasswordHashError> for PasswordError {
    fn from(value: PasswordHashError) -> Self {
        Self::Hash(value)
    }
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;

    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(password_hash)?;
    let argon2 = Argon2::default();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::Password) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn hash_and_verify_round_trip() {
        let password_hash = hash_password("correct horse battery staple").expect("hash succeeds");

        assert!(
            verify_password("correct horse battery staple", &password_hash)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let password_hash = hash_password("correct horse battery staple").expect("hash succeeds");

        assert!(
            !verify_password("tr0ub4dor&3", &password_hash).expect("verify succeeds")
        );
    }
}
