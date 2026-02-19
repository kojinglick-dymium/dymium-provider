//! Configuration management
//!
//! Handles loading and saving configuration from ~/.dymium/config.json

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("Config directory not found")]
    NoDirError,
}

/// Authentication mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AuthMode {
    #[default]
    OAuth,
    StaticKey,
}

/// Token state for the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TokenState {
    Idle,
    Authenticating,
    #[serde(rename_all = "camelCase")]
    Authenticated {
        token: String,
        expires_at: DateTime<Utc>,
    },
    Failed {
        error: String,
    },
}

impl Default for TokenState {
    fn default() -> Self {
        Self::Idle
    }
}

impl TokenState {
    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }

    pub fn is_authenticating(&self) -> bool {
        matches!(self, Self::Authenticating)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Authentication mode: OAuth (Keycloak) or Static API Key
    #[serde(default)]
    pub auth_mode: AuthMode,

    /// LLM endpoint URL (required for both modes)
    #[serde(default)]
    pub llm_endpoint: String,

    // --- OAuth mode fields ---
    #[serde(default)]
    pub keycloak_url: String,

    #[serde(default)]
    pub client_id: String,

    #[serde(default)]
    pub username: String,

    #[serde(default)]
    pub realm: String,

    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_seconds: u64,

    /// The GhostLLM application name or ID (required for OIDC/JWT auth)
    #[serde(default)]
    pub ghostllm_app: Option<String>,

    // OAuth credentials (stored in config for portability, will add keyring later)
    #[serde(default)]
    pub client_secret: Option<String>,

    #[serde(default)]
    pub password: Option<String>,

    #[serde(default)]
    pub refresh_token: Option<String>,

    // --- Static API Key mode fields ---
    #[serde(default)]
    pub static_api_key: Option<String>,
}

fn default_refresh_interval() -> u64 {
    60
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            auth_mode: AuthMode::OAuth,
            llm_endpoint: "http://spoofcorp.llm.dymium.home:9090/v1".to_string(),
            keycloak_url: "https://192.168.50.100:9173".to_string(),
            client_id: "dymium".to_string(),
            username: "dev_mcp_admin@dymium.io".to_string(),
            realm: "dymium".to_string(),
            refresh_interval_seconds: 60,
            ghostllm_app: None,
            client_secret: None,
            password: None,
            refresh_token: None,
            static_api_key: None,
        }
    }
}

impl AppConfig {
    /// Get the config directory path (~/.dymium)
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        dirs::home_dir()
            .map(|p| p.join(".dymium"))
            .ok_or(ConfigError::NoDirError)
    }

    /// Get the config file path (~/.dymium/config.json)
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Get the token file path (~/.dymium/token)
    pub fn token_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("token"))
    }

    /// Load configuration from disk or return defaults
    pub fn load() -> Self {
        Self::try_load().unwrap_or_default()
    }

    /// Try to load configuration from disk
    pub fn try_load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<(), ConfigError> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;

        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get the Keycloak token endpoint URL
    pub fn token_endpoint_url(&self) -> String {
        format!(
            "{}/realms/{}/protocol/openid-connect/token",
            self.keycloak_url, self.realm
        )
    }

    /// Whether using static API key authentication
    pub fn is_static_key_mode(&self) -> bool {
        self.auth_mode == AuthMode::StaticKey
    }

    /// Whether using OAuth authentication
    pub fn is_oauth_mode(&self) -> bool {
        self.auth_mode == AuthMode::OAuth
    }
}
