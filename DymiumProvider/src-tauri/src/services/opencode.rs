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

    /// Get the plugin directory path
    fn plugin_dir() -> Result<PathBuf, OpenCodeError> {
        dirs::home_dir()
            .map(|p| p.join(".local/share/dymium-opencode-plugin"))
            .ok_or(OpenCodeError::NoHomeDir)
    }

    /// Ensure the dymium provider is configured in opencode.json
    pub fn ensure_dymium_provider(config: &AppConfig) -> Result<(), OpenCodeError> {
        // First ensure the plugin exists
        Self::ensure_plugin()?;

        let config_path = Self::config_path()?;

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read existing config or create new
        let mut opencode_config: Value = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            json!({
                "$schema": "https://opencode.ai/config.json"
            })
        };

        let mut changed = false;

        // Get or create provider section
        let providers = opencode_config
            .as_object_mut()
            .unwrap()
            .entry("provider")
            .or_insert_with(|| json!({}));

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
        let plugins = opencode_config
            .as_object_mut()
            .unwrap()
            .entry("plugin")
            .or_insert_with(|| json!([]));

        let plugins_array = plugins.as_array_mut().unwrap();

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

    /// Create or update the OpenCode plugin
    fn ensure_plugin() -> Result<(), OpenCodeError> {
        let plugin_dir = Self::plugin_dir()?;
        fs::create_dir_all(&plugin_dir)?;

        // Write package.json
        let package_json = json!({
            "name": "opencode-dymium-auth",
            "version": "1.0.0",
            "description": "OpenCode plugin for Dymium authentication with automatic token refresh on every request",
            "main": "index.ts",
            "type": "module",
            "keywords": ["opencode", "plugin", "dymium", "auth"],
            "author": "Dymium Provider App",
            "license": "MIT"
        });

        fs::write(
            plugin_dir.join("package.json"),
            serde_json::to_string_pretty(&package_json)?,
        )?;

        // Write the plugin TypeScript code (embedded at compile time)
        // This is a lightweight version — auth is handled via options.apiKey in opencode.json.
        // The plugin provides event logging for debugging the GhostLLM integration.
        const PLUGIN_SOURCE: &str = r#"import fs from "fs"
import path from "path"
import os from "os"

const LOG_DIR = path.join(os.homedir(), ".local/share/dymium-opencode-plugin")
const LOG_FILE = path.join(LOG_DIR, "debug.log")

try { if (!fs.existsSync(LOG_DIR)) fs.mkdirSync(LOG_DIR, { recursive: true }) } catch {}

function log(message: string) {
  try { fs.appendFileSync(LOG_FILE, `${new Date().toISOString()} ${message}\n`) } catch {}
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const s = Math.floor(ms / 1000)
  if (s < 60) return `${s}s`
  return `${Math.floor(s / 60)}m ${s % 60}s`
}

let startTime: number | null = null

export default async function plugin({ client, project, directory }: any) {
  log(`Plugin initialized for project: ${project?.name || directory}`)
  return {
    event: async ({ event }: { event: { type: string; properties?: Record<string, any> } }) => {
      const { type, properties: props = {} } = event
      switch (type) {
        case "session.created":
          startTime = Date.now()
          log("Session created")
          break
        case "session.idle":
          if (startTime) { log(`Session completed in ${formatDuration(Date.now() - startTime)}`); startTime = null }
          break
        case "session.error":
          log(`Session error: ${JSON.stringify(props)}`)
          if (startTime) { log(`Session failed after ${formatDuration(Date.now() - startTime)}`); startTime = null }
          break
        case "session.status":
          log(`Session status: ${props.status || "unknown"}`)
          break
        default:
          if (type.startsWith("session.") || type.startsWith("message.")) log(`Event: ${type}`)
      }
    },
  }
}
"#;
        fs::write(plugin_dir.join("index.ts"), PLUGIN_SOURCE)?;

        log::info!("Dymium OpenCode plugin created at {}", plugin_dir.display());
        Ok(())
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
            serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
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
        let mut auth: Value = serde_json::from_str(&content).unwrap_or_else(|_| json!({}));

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
