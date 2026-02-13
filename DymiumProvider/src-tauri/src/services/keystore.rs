//! Cross-platform secret storage
//!
//! Uses the `keyring` crate for secure credential storage on all platforms:
//! - macOS: Keychain
//! - Linux: Secret Service (GNOME Keyring, KWallet)
//! - Windows: Credential Manager

use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "io.dymium.provider";

#[derive(Error, Debug)]
pub enum KeystoreError {
    #[error("Keyring error: {0}")]
    KeyringError(#[from] keyring::Error),
}

/// Keys for storing secrets
#[derive(Debug, Clone, Copy)]
pub enum CredentialKey {
    ClientSecret,
    Password,
    RefreshToken,
}

impl CredentialKey {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ClientSecret => "client_secret",
            Self::Password => "password",
            Self::RefreshToken => "refresh_token",
        }
    }
}

/// Cross-platform keystore service
pub struct KeystoreService;

impl KeystoreService {
    /// Save a secret to the system keystore
    pub fn save(key: CredentialKey, value: &str) -> Result<(), KeystoreError> {
        let entry = Entry::new(SERVICE_NAME, key.as_str())?;
        entry.set_password(value)?;
        log::debug!("Saved {} to keystore", key.as_str());
        Ok(())
    }

    /// Load a secret from the system keystore
    pub fn load(key: CredentialKey) -> Result<Option<String>, KeystoreError> {
        let entry = Entry::new(SERVICE_NAME, key.as_str())?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeystoreError::KeyringError(e)),
        }
    }

    /// Delete a secret from the system keystore
    pub fn delete(key: CredentialKey) -> Result<(), KeystoreError> {
        let entry = Entry::new(SERVICE_NAME, key.as_str())?;
        match entry.delete_credential() {
            Ok(_) => {
                log::debug!("Deleted {} from keystore", key.as_str());
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
            Err(e) => Err(KeystoreError::KeyringError(e)),
        }
    }

    /// Check if a secret exists in the keystore
    pub fn exists(key: CredentialKey) -> bool {
        Self::load(key).map(|v| v.is_some()).unwrap_or(false)
    }
}
