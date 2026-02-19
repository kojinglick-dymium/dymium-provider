//! Token service
//!
//! Handles OAuth authentication with Keycloak and token management

use crate::services::config::{AppConfig, AuthMode, TokenState};
use crate::services::keystore::{CredentialKey, KeystoreService};
use crate::services::opencode::OpenCodeService;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokenError {
    #[error("Invalid URL")]
    InvalidUrl,
    #[error("Missing client secret")]
    MissingClientSecret,
    #[error("Missing password")]
    MissingPassword,
    #[error("Invalid response")]
    InvalidResponse,
    #[error("Auth failed ({status}): {body}")]
    AuthFailed { status: u16, body: String },
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Keystore error: {0}")]
    KeystoreError(#[from] crate::services::keystore::KeystoreError),
}

/// Response from Keycloak token endpoint
#[derive(Debug, Deserialize)]
struct KeycloakTokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    refresh_expires_in: Option<i64>,
    token_type: String,
}

/// Token service for managing authentication
pub struct TokenService {
    config: AppConfig,
    state: TokenState,
    client: Client,
    last_refresh: Option<chrono::DateTime<Utc>>,
}

impl TokenService {
    pub fn new() -> Self {
        // Create HTTP client that accepts self-signed certificates
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: AppConfig::load(),
            state: TokenState::Idle,
            client,
            last_refresh: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> &TokenState {
        &self.state
    }

    /// Get current config
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Reload config from disk
    pub fn reload_config(&mut self) {
        self.config = AppConfig::load();
    }

    /// Start the token refresh loop (or just set static key)
    pub async fn start_refresh_loop(&mut self) -> Result<(), TokenError> {
        let result = if self.config.is_static_key_mode() {
            self.setup_static_api_key().await
        } else {
            self.authenticate().await
        };

        if let Err(ref e) = result {
            self.state = TokenState::Failed {
                error: e.to_string(),
            };
        }

        result
    }

    /// Set up static API key mode
    async fn setup_static_api_key(&mut self) -> Result<(), TokenError> {
        let api_key = self
            .config
            .static_api_key
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| TokenError::ConfigError("No static API key configured".to_string()))?
            .clone();

        self.state = TokenState::Authenticating;

        // Write the static API key as the token
        self.write_token(&api_key)?;
        log::info!("Static API key written to token file");

        // Ensure OpenCode config and update auth.json
        OpenCodeService::ensure_dymium_provider(&self.config)
            .map_err(|e| TokenError::ConfigError(format!("Failed to update OpenCode config: {}", e)))?;

        log::info!("Updated opencode.json with static API key");

        // Verify the endpoint actually works before declaring success
        self.state = TokenState::Verifying;
        self.verify_endpoint(&api_key).await?;

        // Static keys don't expire, so use a far-future date
        let far_future = Utc::now() + Duration::days(365);
        self.state = TokenState::Authenticated {
            token: api_key,
            expires_at: far_future,
        };
        self.last_refresh = Some(Utc::now());
        log::info!("Static API key verified and authenticated");

        Ok(())
    }

    /// Authenticate with Keycloak
    async fn authenticate(&mut self) -> Result<(), TokenError> {
        self.state = TokenState::Authenticating;

        // Try refresh token first if we have one
        if let Some(refresh_token) = &self.config.refresh_token {
            log::info!("Attempting refresh token grant...");
            match self.perform_refresh_token_grant(refresh_token.clone()).await {
                Ok(response) => {
                    log::info!(
                        "Refresh token grant succeeded, token expires in {}s",
                        response.expires_in
                    );
                    self.handle_successful_auth(response).await?;
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("Refresh token grant failed: {}", e);
                    log::info!("Falling back to password grant...");
                }
            }
        } else {
            log::info!("No refresh token found, using password grant");
        }

        // Fall back to password grant
        let response = self.perform_password_grant().await?;
        log::info!(
            "Password grant succeeded, token expires in {}s",
            response.expires_in
        );
        self.handle_successful_auth(response).await?;

        Ok(())
    }

    /// Handle successful authentication response
    async fn handle_successful_auth(&mut self, response: KeycloakTokenResponse) -> Result<(), TokenError> {
        let expires_at = Utc::now() + Duration::seconds(response.expires_in);

        // Store refresh token if we got one
        if let Some(ref refresh_token) = response.refresh_token {
            self.config.refresh_token = Some(refresh_token.clone());
            if let Err(e) = self.config.save() {
                log::error!("Failed to save refresh token: {}", e);
            }
            if let Some(expires_in) = response.refresh_expires_in {
                log::info!("New refresh token saved, expires in {}s", expires_in);
            }
        }

        // Write access token to disk
        self.write_token(&response.access_token)?;
        log::info!("Access token written to token file");

        // Ensure OpenCode config and update auth.json
        OpenCodeService::ensure_dymium_provider(&self.config)
            .map_err(|e| TokenError::ConfigError(format!("Failed to update OpenCode config: {}", e)))?;

        log::info!("Updated opencode.json with OAuth token, expires at {}", expires_at);

        // Verify the endpoint actually works
        self.state = TokenState::Verifying;
        self.verify_endpoint(&response.access_token).await?;

        self.state = TokenState::Authenticated {
            token: response.access_token,
            expires_at,
        };
        self.last_refresh = Some(Utc::now());

        Ok(())
    }

    /// Verify the LLM endpoint is reachable and accepts our token.
    /// Uses the same effective URL that OpenCode will use (with app path for OIDC).
    async fn verify_endpoint(&self, token: &str) -> Result<(), TokenError> {
        let effective_url = OpenCodeService::compute_base_url(&self.config);
        let effective_trimmed = effective_url.trim_end_matches('/');

        // Build the models URL from the effective base
        let models_url = if effective_trimmed.ends_with("/v1") {
            format!("{}/models", effective_trimmed)
        } else {
            format!("{}/v1/models", effective_trimmed)
        };

        log::info!("Verifying endpoint: GET {}", models_url);

        let response = self
            .client
            .get(&models_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Host", extract_hostname(&models_url))
            .send()
            .await
            .map_err(|e| {
                let msg = if e.is_connect() {
                    format!("Cannot reach LLM endpoint ({})", effective_trimmed)
                } else if e.is_timeout() {
                    format!("LLM endpoint timed out ({})", effective_trimmed)
                } else {
                    format!("LLM endpoint error: {}", e)
                };
                TokenError::ConfigError(msg)
            })?;

        let status = response.status();
        if status.is_success() {
            log::info!("Endpoint verified: {} returned {}", models_url, status);
            Ok(())
        } else if status.as_u16() == 401 {
            let body = response.text().await.unwrap_or_default();
            log::warn!("Endpoint rejected token: {} {}", status, body);
            Err(TokenError::ConfigError(
                "LLM endpoint rejected the API key (401 Unauthorized)".to_string(),
            ))
        } else {
            let body = response.text().await.unwrap_or_default();
            log::warn!("Endpoint returned {}: {}", status, body);
            Err(TokenError::ConfigError(format!(
                "LLM endpoint returned {} — check endpoint URL",
                status
            )))
        }
    }

    /// Perform password grant authentication
    async fn perform_password_grant(&self) -> Result<KeycloakTokenResponse, TokenError> {
        let url = &self.config.token_endpoint_url();

        let client_secret = self
            .config
            .client_secret
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or(TokenError::MissingClientSecret)?;

        let password = self
            .config
            .password
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or(TokenError::MissingPassword)?;

        let params = [
            ("grant_type", "password"),
            ("client_id", &self.config.client_id),
            ("client_secret", client_secret),
            ("username", &self.config.username),
            ("password", password),
        ];

        let response = self.client.post(url).form(&params).send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TokenError::AuthFailed {
                status: status.as_u16(),
                body,
            });
        }

        let token_response: KeycloakTokenResponse = response.json().await?;
        Ok(token_response)
    }

    /// Perform refresh token grant
    async fn perform_refresh_token_grant(
        &mut self,
        refresh_token: String,
    ) -> Result<KeycloakTokenResponse, TokenError> {
        let url = &self.config.token_endpoint_url();

        let client_secret = self
            .config
            .client_secret
            .as_ref()
            .filter(|s| !s.is_empty())
            .ok_or(TokenError::MissingClientSecret)?;

        let params = [
            ("grant_type", "refresh_token"),
            ("client_id", &self.config.client_id),
            ("client_secret", client_secret),
            ("refresh_token", &refresh_token),
        ];

        let response = self.client.post(url).form(&params).send().await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            log::warn!(
                "Refresh token grant failed with status {}: {}",
                status.as_u16(),
                body
            );

            // Only clear refresh token if definitively invalid (400/401)
            if status.as_u16() == 400 || status.as_u16() == 401 {
                log::info!("Clearing invalid refresh token");
                self.config.refresh_token = None;
                let _ = self.config.save();
            }

            return Err(TokenError::AuthFailed {
                status: status.as_u16(),
                body,
            });
        }

        let token_response: KeycloakTokenResponse = response.json().await?;
        Ok(token_response)
    }

    /// Write token to disk
    fn write_token(&self, token: &str) -> Result<(), TokenError> {
        let path = AppConfig::token_path()
            .map_err(|e| TokenError::ConfigError(e.to_string()))?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write token
        fs::write(&path, token)?;

        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        log::info!("Token written to {}", path.display());
        Ok(())
    }

    /// Manually trigger a refresh
    pub async fn manual_refresh(&mut self) -> Result<(), TokenError> {
        let result = if self.config.is_static_key_mode() {
            self.setup_static_api_key().await
        } else {
            self.authenticate().await
        };

        if let Err(ref e) = result {
            self.state = TokenState::Failed {
                error: e.to_string(),
            };
        }

        result
    }

    /// Log out - clear all stored credentials and tokens
    pub fn log_out(&mut self) -> Result<(), TokenError> {
        // Clear credentials from config
        self.config.client_secret = None;
        self.config.password = None;
        self.config.refresh_token = None;
        self.config.static_api_key = None;
        self.config.save().map_err(|e| TokenError::ConfigError(e.to_string()))?;

        // Delete keystore entries
        let _ = KeystoreService::delete(CredentialKey::ClientSecret);
        let _ = KeystoreService::delete(CredentialKey::Password);
        let _ = KeystoreService::delete(CredentialKey::RefreshToken);

        // Delete token file
        if let Ok(path) = AppConfig::token_path() {
            let _ = fs::remove_file(path);
        }

        // Delete auth.json for OpenCode
        if let Some(data_dir) = dirs::data_local_dir() {
            let auth_path = data_dir.join("opencode").join("auth.json");
            let _ = fs::remove_file(auth_path);
        }

        // Reset state
        self.state = TokenState::Idle;
        self.last_refresh = None;

        log::info!("Logged out - all credentials cleared");
        Ok(())
    }

    /// Save OAuth configuration
    pub fn save_oauth_setup(
        &mut self,
        keycloak_url: String,
        realm: String,
        client_id: String,
        username: String,
        llm_endpoint: String,
        ghostllm_app: Option<String>,
        client_secret: String,
        password: String,
    ) -> Result<(), TokenError> {
        // Clear old credentials immediately when switching modes
        self.clear_cached_credentials();
        
        self.config.auth_mode = AuthMode::OAuth;
        self.config.keycloak_url = keycloak_url;
        self.config.realm = realm;
        self.config.client_id = client_id;
        self.config.username = username;
        self.config.llm_endpoint = llm_endpoint;
        self.config.ghostllm_app = ghostllm_app;
        self.config.client_secret = Some(client_secret);
        self.config.password = Some(password);
        self.config.refresh_token = None; // Clear old refresh token
        self.config.static_api_key = None;

        self.config.save().map_err(|e| TokenError::ConfigError(e.to_string()))?;
        log::info!("OAuth configuration saved");
        Ok(())
    }

    /// Save static API key configuration
    pub fn save_static_key_setup(
        &mut self,
        llm_endpoint: String,
        static_api_key: String,
        ghostllm_app: Option<String>,
    ) -> Result<(), TokenError> {
        // Clear old credentials immediately when switching modes
        self.clear_cached_credentials();
        
        self.config.auth_mode = AuthMode::StaticKey;
        self.config.llm_endpoint = llm_endpoint;
        self.config.static_api_key = Some(static_api_key);
        self.config.ghostllm_app = ghostllm_app;
        self.config.client_secret = None;
        self.config.password = None;
        self.config.refresh_token = None;

        self.config.save().map_err(|e| TokenError::ConfigError(e.to_string()))?;
        log::info!("Static API key configuration saved");
        Ok(())
    }

    /// Clear cached credentials (token file and auth.json)
    /// Called when switching auth modes to prevent stale credentials from being used
    fn clear_cached_credentials(&self) {
        // Delete token file
        if let Ok(path) = AppConfig::token_path() {
            if let Err(e) = fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to remove token file: {}", e);
                }
            } else {
                log::info!("Cleared token file");
            }
        }

        // Clear dymium entry from auth.json
        OpenCodeService::clear_dymium_auth();
    }

    /// Check if credentials are configured
    pub fn has_credentials(&self) -> bool {
        if self.config.is_static_key_mode() {
            self.config
                .static_api_key
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        } else {
            self.config
                .client_secret
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
                && self
                    .config
                    .password
                    .as_ref()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
        }
    }
}

/// Extract hostname from a URL string for the Host header.
/// Returns just the hostname without port (for Istio VirtualService matching).
fn extract_hostname(url: &str) -> String {
    // Strip scheme
    let after_scheme = url
        .find("://")
        .map(|i| &url[i + 3..])
        .unwrap_or(url);
    // Take up to first / or :
    let host = after_scheme
        .split(&['/', ':'][..])
        .next()
        .unwrap_or("localhost");
    host.to_string()
}
