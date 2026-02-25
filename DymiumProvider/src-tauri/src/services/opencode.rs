//! OpenCode configuration management
//!
//! Handles updating the OpenCode config (~/.config/opencode/opencode.json)
//! and auth file (~/.local/share/opencode/auth.json)

use crate::services::config::AppConfig;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OpenCodeError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Config parse error: {0}")]
    ParseError(String),
    #[error("Home directory not found")]
    NoHomeDir,
}

/// Service for managing OpenCode configuration
pub struct OpenCodeService;

impl OpenCodeService {
    /// Get the OpenCode config path
    /// OpenCode (Node.js) uses ~/.config/opencode/ on all platforms (XDG convention),
    /// NOT the platform-native config dir (~/Library/Application Support on macOS).
    fn config_path() -> Result<PathBuf, OpenCodeError> {
        dirs::home_dir()
            .map(|p| p.join(".config/opencode/opencode.json"))
            .ok_or(OpenCodeError::NoHomeDir)
    }

    /// Get the OpenCode auth path
    /// OpenCode (Node.js) uses ~/.local/share/opencode/ on all platforms (XDG convention),
    /// NOT the platform-native data dir (~/Library/Application Support on macOS).
    fn auth_path() -> Result<PathBuf, OpenCodeError> {
        dirs::home_dir()
            .map(|p| p.join(".local/share/opencode/auth.json"))
            .ok_or(OpenCodeError::NoHomeDir)
    }

    /// Ensure the dymium provider is configured in opencode.json
    pub fn ensure_dymium_provider(config: &AppConfig) -> Result<(), OpenCodeError> {
        let config_path = Self::config_path()?;

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read existing config (accept JSON with comments/trailing commas) or create new
        let mut opencode_config: Value = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            Self::parse_json_like(&content)?
        } else {
            json!({
                "$schema": "https://opencode.ai/config.json"
            })
        };

        if !opencode_config.is_object() {
            log::warn!("opencode.json root was not an object; recreating object root");
            opencode_config = json!({
                "$schema": "https://opencode.ai/config.json"
            });
        }

        let mut changed = false;

        // Get or create provider section
        let providers = opencode_config
            .as_object_mut()
            .unwrap()
            .entry("provider")
            .or_insert_with(|| json!({}));
        if !providers.is_object() {
            log::warn!("opencode.json 'provider' key was not an object; replacing with object");
            *providers = json!({});
            changed = true;
        }

        // Resolve the API key to write into options.apiKey
        let api_key = Self::resolve_token(config).ok();

        // Compute the effective baseURL, injecting the app path when configured.
        // GhostLLM routes: /{app}/v1/chat/completions (preferred, required for OIDC)
        // vs legacy: /v1/chat/completions (static key only, app inferred from key)
        //
        // User enters endpoint like: http://host:9090/v1
        // With ghostllm_app "myapp": http://host:9090/myapp/v1
        let effective_base_url = Self::compute_base_url(config);

        // Add or update dymium provider
        let providers_map = providers.as_object_mut().unwrap();
        if let Some(existing) = providers_map.get_mut("dymium") {
            if !existing.is_object() {
                log::warn!("opencode.json provider.dymium was not an object; replacing with object");
                *existing = json!({});
                changed = true;
            }
            let obj = existing.as_object_mut().unwrap();

            // Always update `api` field to the effective URL
            let current_api = obj
                .get("api")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            if current_api != effective_base_url {
                obj.insert("api".to_string(), json!(&effective_base_url));
                changed = true;
                log::info!(
                    "Updated dymium provider api in opencode.json: {} -> {}",
                    current_api,
                    effective_base_url
                );
            }

            // Merge into existing options (preserve user-set headers, etc.)
            let options = obj.entry("options").or_insert_with(|| json!({}));
            if !options.is_object() {
                log::warn!("opencode.json provider.dymium.options was not an object; replacing with object");
                *options = json!({});
                changed = true;
            }
            let opts = options.as_object_mut().unwrap();

            // Update baseURL if changed
            let current_base = opts
                .get("baseURL")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
            if current_base != effective_base_url {
                opts.insert("baseURL".to_string(), json!(&effective_base_url));
                changed = true;
                log::info!(
                    "Updated dymium provider baseURL in opencode.json: {} -> {}",
                    current_base,
                    effective_base_url
                );
            }

            // Update apiKey if changed (this is how OpenCode actually reads auth)
            if let Some(ref key) = api_key {
                let current_key = opts
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                if current_key != *key {
                    opts.insert("apiKey".to_string(), json!(key));
                    changed = true;
                    log::info!("Updated dymium provider apiKey in opencode.json");
                }
            }
        } else {
            let mut options = json!({
                "baseURL": &effective_base_url
            });
            if let Some(ref key) = api_key {
                options
                    .as_object_mut()
                    .unwrap()
                    .insert("apiKey".to_string(), json!(key));
            }

            providers_map.insert(
                "dymium".to_string(),
                json!({
                    "npm": "@ai-sdk/openai-compatible",
                    "name": "Dymium",
                    "api": &effective_base_url,
                    "options": options,
                    "models": {
                        "claude-opus-4-5": {
                            "name": "Claude Opus 4.5 (via Dymium)",
                            "tool_call": true,
                            "temperature": true,
                            "attachment": true,
                            "reasoning": true,
                            "interleaved": { "field": "reasoning_content" },
                            "limit": {
                                "context": 200000,
                                "output": 16384
                            }
                        },
                        "claude-sonnet-4": {
                            "name": "Claude Sonnet 4 (via Dymium)",
                            "tool_call": true,
                            "temperature": true,
                            "attachment": true,
                            "reasoning": false,
                            "limit": {
                                "context": 200000,
                                "output": 16384
                            }
                        }
                    }
                }),
            );
            changed = true;
            log::info!("Added dymium provider to opencode.json");
        }

        // Ensure plugin is registered via npm
        let npm_plugin = "dymium-auth-plugin@latest";
        let plugins_value = opencode_config
            .as_object_mut()
            .unwrap()
            .entry("plugin")
            .or_insert_with(|| json!([]));
        let plugins_array = match plugins_value {
            Value::Array(arr) => arr,
            Value::String(s) => {
                let moved = s.clone();
                *plugins_value = json!([moved]);
                changed = true;
                plugins_value.as_array_mut().unwrap()
            }
            Value::Null => {
                *plugins_value = json!([]);
                changed = true;
                plugins_value.as_array_mut().unwrap()
            }
            _ => {
                log::warn!("opencode.json 'plugin' key was not an array/string; replacing with array");
                *plugins_value = json!([]);
                changed = true;
                plugins_value.as_array_mut().unwrap()
            }
        };

        // Remove any stale file:// plugin entries
        let old_len = plugins_array.len();
        plugins_array.retain(|p| {
            !p.as_str()
                .map(|s| s.contains("dymium-opencode-plugin"))
                .unwrap_or(false)
        });
        if plugins_array.len() != old_len {
            changed = true;
            log::info!("Removed stale file:// dymium plugin entry");
        }

        // Add npm plugin if not already present
        if !plugins_array.iter().any(|p| {
            p.as_str()
                .map(|s| s.contains("dymium-auth-plugin"))
                .unwrap_or(false)
        }) {
            plugins_array.push(json!(npm_plugin));
            changed = true;
            log::info!("Registered dymium auth plugin via npm: {}", npm_plugin);
        }

        // Write config if changed
        if changed {
            let content = serde_json::to_string_pretty(&opencode_config)?;
            fs::write(&config_path, content)?;
            log::info!("Updated {}", config_path.display());
        }

        // Update auth.json
        Self::update_auth_json(config)?;

        Ok(())
    }

    fn parse_json_like(content: &str) -> Result<Value, OpenCodeError> {
        match serde_json::from_str(content) {
            Ok(v) => Ok(v),
            Err(_) => {
                let parsed: Value = json5::from_str(content).map_err(|e| {
                    OpenCodeError::ParseError(format!(
                        "failed to parse as JSON/JSONC/JSON5: {}",
                        e
                    ))
                })?;
                Ok(parsed)
            }
        }
    }

    /// Update the auth.json file with the current token
    fn update_auth_json(config: &AppConfig) -> Result<(), OpenCodeError> {
        // Resolve the token: try the token file first, fall back to static key from config
        let token = Self::resolve_token(config)?;
        Self::write_auth_json(config, &token)
    }

    /// Resolve the current token from available sources
    fn resolve_token(config: &AppConfig) -> Result<String, OpenCodeError> {
        // Try reading from the token file first
        if let Ok(token_path) = AppConfig::token_path() {
            if let Ok(content) = fs::read_to_string(&token_path) {
                let token = content.trim().to_string();
                if !token.is_empty() {
                    return Ok(token);
                }
            }
        }

        // Fall back to static API key from config if in static key mode
        if config.is_static_key_mode() {
            if let Some(ref key) = config.static_api_key {
                if !key.is_empty() {
                    return Ok(key.clone());
                }
            }
        }

        Err(OpenCodeError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No token available (token file missing and no static API key configured)",
        )))
    }

    /// Compute the effective baseURL for the OpenCode provider.
    ///
    /// For OIDC auth, the app name MUST be in the URL path:
    ///   http://host:9090/myapp/v1  →  /{app}/v1/chat/completions
    ///
    /// For static key auth, the legacy path works (server infers app from key):
    ///   http://host:9090/v1  →  /v1/chat/completions
    pub fn compute_base_url(config: &AppConfig) -> String {
        let endpoint = config.llm_endpoint.trim_end_matches('/');

        if config.is_oauth_mode() {
            if let Some(ref app) = config.ghostllm_app {
                let app = app.trim();
                if !app.is_empty() {
                    // Insert app before /v1 in the endpoint
                    // e.g. http://host:9090/v1 → http://host:9090/myapp/v1
                    if let Some(pos) = endpoint.rfind("/v1") {
                        let mut url = String::with_capacity(endpoint.len() + app.len() + 1);
                        url.push_str(&endpoint[..pos]);
                        url.push('/');
                        url.push_str(app);
                        url.push_str(&endpoint[pos..]);
                        log::info!(
                            "OIDC mode: injected app path into baseURL: {} -> {}",
                            endpoint,
                            url
                        );
                        return url;
                    }
                    // Endpoint doesn't contain /v1 — append /{app}/v1
                    let url = format!("{}/{}/v1", endpoint, app);
                    log::info!(
                        "OIDC mode: appended app path to baseURL: {} -> {}",
                        endpoint,
                        url
                    );
                    return url;
                }
            }
        }

        // Static key mode or no app configured — use endpoint as-is
        endpoint.to_string()
    }

    /// Write the dymium entry to auth.json
    fn write_auth_json(config: &AppConfig, token: &str) -> Result<(), OpenCodeError> {
        let auth_path = Self::auth_path()?;

        // Ensure directory exists
        if let Some(parent) = auth_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read existing auth or create new
        let mut auth: Value = if auth_path.exists() {
            let content = fs::read_to_string(&auth_path)?;
            Self::parse_json_like(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        // Determine auth type based on config mode
        let auth_type = if config.is_static_key_mode() {
            "static"
        } else {
            "oauth"
        };

        let mut dymium_auth = json!({
            "type": auth_type,
            "key": token,
            "endpoint": config.llm_endpoint
        });

        // Add ghostllm_app if configured
        if let Some(ref app) = config.ghostllm_app {
            if !app.is_empty() {
                dymium_auth
                    .as_object_mut()
                    .unwrap()
                    .insert("app".to_string(), json!(app));
                log::debug!("Including GhostLLM app in auth.json: {}", app);
            }
        }

        auth.as_object_mut()
            .unwrap()
            .insert("dymium".to_string(), dymium_auth);

        fs::write(&auth_path, serde_json::to_string_pretty(&auth)?)?;
        log::info!(
            "Updated dymium token in {} (mode: {})",
            auth_path.display(),
            auth_type
        );

        Ok(())
    }

    /// Clear the dymium entry from auth.json
    /// Called when switching auth modes to prevent stale credentials
    pub fn clear_dymium_auth() {
        if let Err(e) = Self::do_clear_dymium_auth() {
            log::error!("Failed to clear dymium auth: {}", e);
        }
    }

    fn do_clear_dymium_auth() -> Result<(), OpenCodeError> {
        let auth_path = Self::auth_path()?;

        if !auth_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&auth_path)?;
        let mut auth: Value = Self::parse_json_like(&content).unwrap_or_else(|_| json!({}));

        // Remove the dymium entry
        if let Some(obj) = auth.as_object_mut() {
            if obj.remove("dymium").is_some() {
                fs::write(&auth_path, serde_json::to_string_pretty(&auth)?)?;
                log::info!("Cleared dymium entry from auth.json");
            }
        }

        Ok(())
    }
}
