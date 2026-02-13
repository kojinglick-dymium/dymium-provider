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
    fn config_path() -> Result<PathBuf, OpenCodeError> {
        dirs::config_dir()
            .map(|p| p.join("opencode").join("opencode.json"))
            .ok_or(OpenCodeError::NoHomeDir)
    }

    /// Get the OpenCode auth path
    fn auth_path() -> Result<PathBuf, OpenCodeError> {
        dirs::data_local_dir()
            .map(|p| p.join("opencode").join("auth.json"))
            .ok_or(OpenCodeError::NoHomeDir)
    }

    /// Get the plugin directory path
    fn plugin_dir() -> Result<PathBuf, OpenCodeError> {
        dirs::data_local_dir()
            .map(|p| p.join("dymium-opencode-plugin"))
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

        // Add dymium provider if missing
        if providers.get("dymium").is_none() {
            providers.as_object_mut().unwrap().insert(
                "dymium".to_string(),
                json!({
                    "npm": "@ai-sdk/openai-compatible",
                    "name": "Dymium",
                    "api": config.llm_endpoint,
                    "options": {
                        "baseURL": config.llm_endpoint
                    },
                    "models": {
                        "claude-opus-4-5": {
                            "name": "Claude Opus 4.5 (via Dymium)",
                            "tool_call": true,
                            "temperature": true,
                            "attachment": true,
                            "reasoning": true,
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

        // Ensure plugin is registered
        let plugin_url = format!("file://{}", Self::plugin_dir()?.display());
        let plugins = opencode_config
            .as_object_mut()
            .unwrap()
            .entry("plugin")
            .or_insert_with(|| json!([]));

        let plugins_array = plugins.as_array_mut().unwrap();
        if !plugins_array.iter().any(|p| {
            p.as_str()
                .map(|s| s.contains("dymium-opencode-plugin"))
                .unwrap_or(false)
        }) {
            plugins_array.push(json!(plugin_url));
            changed = true;
            log::info!("Registered dymium auth plugin: {}", plugin_url);
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
        // The path is relative to the source file location
        const PLUGIN_SOURCE: &str = r#"import fs from "fs"
import path from "path"
import os from "os"
import http from "http"
import https from "https"

// Path to the auth.json file
const AUTH_JSON_PATH = path.join(os.homedir(), ".local/share/opencode/auth.json")

// Log file for debugging (no console.log to avoid polluting OpenCode UI)
const LOG_FILE = path.join(os.homedir(), ".local/share/dymium-opencode-plugin/debug.log")

function log(message: string) {
  const timestamp = new Date().toISOString()
  const line = `${timestamp} ${message}\n`
  try {
    fs.appendFileSync(LOG_FILE, line)
  } catch {}
}

function getDymiumToken(): string | null {
  try {
    if (!fs.existsSync(AUTH_JSON_PATH)) {
      log(`auth.json not found at ${AUTH_JSON_PATH}`)
      return null
    }
    const content = fs.readFileSync(AUTH_JSON_PATH, "utf-8")
    const auth = JSON.parse(content)
    if (auth.dymium?.key) {
      return auth.dymium.key
    }
    log("No dymium.key found in auth.json")
    return null
  } catch (error) {
    log(`Failed to read auth.json: ${error}`)
    return null
  }
}

function http11Request(
  url: URL,
  options: { method: string; headers: Record<string, string>; body?: string }
): Promise<Response> {
  return new Promise((resolve, reject) => {
    const isHttps = url.protocol === "https:"
    const lib = isHttps ? https : http
    const hostHeader = url.hostname
    const reqOptions: http.RequestOptions = {
      hostname: url.hostname,
      port: url.port || (isHttps ? 443 : 80),
      path: url.pathname + url.search,
      method: options.method,
      headers: { ...options.headers, "Host": hostHeader, "Connection": "close" },
    }
    if (options.body) {
      reqOptions.headers!["Content-Length"] = Buffer.byteLength(options.body).toString()
    }
    log(`HTTP/1.1 ${options.method} ${url.toString()} Host: ${hostHeader}`)
    const req = lib.request(reqOptions, (res) => {
      const chunks: Buffer[] = []
      res.on("data", (chunk) => chunks.push(chunk))
      res.on("end", () => {
        const body = Buffer.concat(chunks)
        const responseHeaders = new Headers()
        for (const [key, value] of Object.entries(res.headers)) {
          if (value) {
            if (Array.isArray(value)) {
              value.forEach(v => responseHeaders.append(key, v))
            } else {
              responseHeaders.set(key, value)
            }
          }
        }
        log(`Response: ${res.statusCode} ${res.statusMessage}`)
        resolve(new Response(body, {
          status: res.statusCode || 200,
          statusText: res.statusMessage || "",
          headers: responseHeaders,
        }))
      })
    })
    req.on("error", (err) => { log(`Request error: ${err.message}`); reject(err) })
    req.setTimeout(120000, () => { log("Request timeout"); req.destroy(new Error("Request timeout")) })
    if (options.body) req.write(options.body)
    req.end()
  })
}

async function dymiumFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const token = getDymiumToken()
  if (!token) {
    throw new Error("[dymium-auth] No valid Dymium token available. Please ensure the Dymium Provider app is running.")
  }
  const url = typeof input === "string" ? new URL(input) : input instanceof URL ? input : new URL(input.url)
  const headers: Record<string, string> = { "Content-Type": "application/json", "Accept": "application/json, text/event-stream" }
  if (init?.headers) {
    const initHeaders = new Headers(init.headers)
    initHeaders.forEach((value, key) => { headers[key] = value })
  }
  headers["Authorization"] = `Bearer ${token}`
  let body: string | undefined
  if (init?.body) {
    if (typeof init.body === "string") body = init.body
    else if (init.body instanceof ArrayBuffer) body = new TextDecoder().decode(init.body)
    else if (ArrayBuffer.isView(init.body)) body = new TextDecoder().decode(init.body)
    else body = String(init.body)
  }
  return http11Request(url, { method: init?.method || "GET", headers, body })
}

export default async function plugin({ client, project, directory }: any) {
  log(`Plugin initialized for project: ${project?.name || directory}`)
  return {
    auth: {
      provider: "dymium",
      methods: [],
      async loader(getAuth: () => Promise<any>, provider: any) {
        log(`Loader called for provider: ${provider?.id || provider}`)
        return { apiKey: "", fetch: dymiumFetch }
      },
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

        // Read the current token
        let token_path = AppConfig::token_path().map_err(|_| OpenCodeError::NoHomeDir)?;
        let token = fs::read_to_string(&token_path).ok();

        if let Some(token) = token {
            let token = token.trim().to_string();
            let mut dymium_auth = json!({
                "type": "api",
                "key": token
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
            log::info!("Updated dymium token in {}", auth_path.display());
        } else {
            log::warn!("No token available to write to auth.json");
        }

        Ok(())
    }

    /// Force update the auth token (called on every refresh)
    pub fn update_token(config: &AppConfig) {
        if let Err(e) = Self::update_auth_json(config) {
            log::error!("Failed to update auth.json: {}", e);
        }
    }
}
