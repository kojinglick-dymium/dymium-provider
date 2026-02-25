//! Dymium Provider - Cross-platform GhostLLM authentication manager
//!
//! This application runs as a system tray app and manages authentication tokens
//! for GhostLLM. It supports both OAuth (Keycloak) and static API key authentication.

mod services;

use services::config::{AppConfig, TokenState};
use services::opencode::OpenCodeService;
use services::token::TokenService;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tokio::sync::Mutex;

/// Shared application state
pub struct AppState {
    pub token_service: Arc<Mutex<TokenService>>,
}

/// Get current token state
#[tauri::command]
async fn get_state(state: State<'_, AppState>) -> Result<TokenState, String> {
    let service = state.token_service.lock().await;
    Ok(service.state().clone())
}

/// Get current configuration
#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let service = state.token_service.lock().await;
    Ok(service.config().clone())
}

/// Save OAuth configuration
#[tauri::command]
async fn save_oauth_config(
    app: AppHandle,
    state: State<'_, AppState>,
    keycloak_url: String,
    realm: String,
    client_id: String,
    username: String,
    llm_endpoint: String,
    ghostllm_app: Option<String>,
    client_secret: String,
    password: String,
) -> Result<(), String> {
    let mut service = state.token_service.lock().await;
    let result = service
        .save_oauth_setup(
            keycloak_url,
            realm,
            client_id,
            username,
            llm_endpoint,
            ghostllm_app,
            client_secret,
            password,
        );
    update_tray_status(&app, service.state());
    let _ = app.emit("token-state-changed", service.state());
    result.map_err(|e| e.to_string())
}

/// Save static API key configuration
#[tauri::command]
async fn save_static_key_config(
    app: AppHandle,
    state: State<'_, AppState>,
    llm_endpoint: String,
    static_api_key: String,
    ghostllm_app: Option<String>,
) -> Result<(), String> {
    let mut service = state.token_service.lock().await;
    let result = service.save_static_key_setup(llm_endpoint, static_api_key, ghostllm_app);
    update_tray_status(&app, service.state());
    let _ = app.emit("token-state-changed", service.state());
    result.map_err(|e| e.to_string())
}

/// Manually trigger a token refresh
#[tauri::command]
async fn manual_refresh(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut service = state.token_service.lock().await;
    let result = service.manual_refresh().await;
    update_tray_status(&app, service.state());
    let _ = app.emit("token-state-changed", service.state());
    result.map_err(|e| e.to_string())
}

/// Log out and clear all credentials
#[tauri::command]
async fn log_out(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut service = state.token_service.lock().await;
    let result = service.log_out();
    update_tray_status(&app, service.state());
    let _ = app.emit("token-state-changed", service.state());
    result.map_err(|e| e.to_string())
}

/// Check if credentials are configured
#[tauri::command]
async fn has_credentials(state: State<'_, AppState>) -> Result<bool, String> {
    let service = state.token_service.lock().await;
    Ok(service.has_credentials())
}

/// Start the token refresh loop
#[tauri::command]
async fn start_refresh_loop(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut service = state.token_service.lock().await;
    let result = service.start_refresh_loop().await;
    update_tray_status(&app, service.state());
    let _ = app.emit("token-state-changed", service.state());
    result.map_err(|e| e.to_string())
}

/// Build the tray menu
fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let status = MenuItem::with_id(app, "status", "Status: Initializing...", false, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh Now", true, None::<&str>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let setup = MenuItem::with_id(app, "setup", "Setup...", true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[&status, &refresh, &separator1, &setup, &separator2, &quit])
}

/// Update tray menu status text
fn update_tray_status(app: &AppHandle, state: &TokenState) {
    let status_text = match state {
        TokenState::Idle => "Status: Not configured".to_string(),
        TokenState::Authenticating => "Status: Connecting...".to_string(),
        TokenState::Verifying => "Status: Verifying endpoint...".to_string(),
        TokenState::Authenticated { expires_at, .. } => {
            format!("Status: Connected (expires {})", expires_at.format("%H:%M"))
        }
        TokenState::Failed { error } => {
            let normalized = error.to_lowercase();
            if normalized.contains("401")
                || normalized.contains("unauthorized")
                || normalized.contains("invalid api key")
                || normalized.contains("invalid oidc token")
            {
                "Status: Unauthorized".to_string()
            } else if normalized.contains("timed out") {
                "Status: Endpoint timeout".to_string()
            } else if normalized.contains("cannot reach llm endpoint") {
                "Status: Endpoint unreachable".to_string()
            } else if normalized.contains("failed to update opencode config") {
                "Status: OpenCode config error".to_string()
            } else {
                "Status: Error".to_string()
            }
        }
    };

    // Update the menu item text
    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(menu) = build_tray_menu(app) {
            // Update status item
            if let Some(item) = menu.get("status") {
                if let Some(menu_item) = item.as_menuitem() {
                    let _ = menu_item.set_text(&status_text);
                }
            }
            let _ = tray.set_menu(Some(menu));
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Create the token service
            let token_service = Arc::new(Mutex::new(TokenService::new()));

            // Store in app state
            app.manage(AppState {
                token_service: token_service.clone(),
            });

            // Build the tray menu
            let menu = build_tray_menu(app.handle())?;

            // Load tray icon from embedded PNG bytes (44x44 for retina displays)
            let icon_bytes = include_bytes!("../icons/tray-icon.png");
            let icon = tauri::image::Image::from_bytes(icon_bytes)
                .expect("Failed to load tray icon");

            // Create the tray icon
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                .icon_as_template(true) // Use as template for macOS menu bar (respects dark/light mode)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "refresh" => {
                            let app = app.clone();
                            let ts = token_service.clone();
                            tauri::async_runtime::spawn(async move {
                                let mut service = ts.lock().await;
                                if let Err(e) = service.manual_refresh().await {
                                    log::error!("Manual refresh failed: {}", e);
                                }
                                // Update tray status
                                update_tray_status(&app, service.state());
                                // Emit event to frontend
                                let _ = app.emit("token-state-changed", service.state());
                            });
                        }
                        "setup" => {
                            // Show the setup window
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        // Left click shows menu (default behavior)
                    }
                })
                .build(app)?;

            // Initialize tray status immediately from current in-memory state.
            {
                let ts = app.state::<AppState>().token_service.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let service = ts.lock().await;
                    update_tray_status(&app_handle, service.state());
                });
            }

            // Sync managed files and start token refresh loop in background
            let app_handle = app.handle().clone();
            let ts = app.state::<AppState>().token_service.clone();
            tauri::async_runtime::spawn(async move {
                // --- Initial authentication ---
                {
                    let mut service = ts.lock().await;

                    // Always sync opencode.json and auth.json on startup
                    let config = service.config().clone();
                    if let Err(e) = OpenCodeService::ensure_dymium_provider(&config) {
                        log::warn!("Failed to sync OpenCode config on startup: {}", e);
                    }

                    if service.has_credentials() {
                        log::info!("Starting initial authentication...");
                        if let Err(e) = service.start_refresh_loop().await {
                            log::error!("Failed initial authentication: {}", e);
                        }
                    }
                    update_tray_status(&app_handle, service.state());
                    let _ = app_handle.emit("token-state-changed", service.state());
                }
                // Lock released here — periodic loop can proceed independently

                // --- Periodic OAuth token refresh ---
                // Only runs for OAuth mode; static keys don't expire.
                loop {
                    let interval_secs = {
                        let service = ts.lock().await;
                        if !service.needs_refresh_loop() {
                            // Not OAuth or not authenticated — park until
                            // something changes (manual refresh / re-save)
                            drop(service);
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            continue;
                        }
                        service.refresh_interval_secs()
                    };

                    tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

                    let mut service = ts.lock().await;
                    if !service.needs_refresh_loop() {
                        continue;
                    }
                    match service.refresh_tick().await {
                        Ok(()) => {
                            update_tray_status(&app_handle, service.state());
                            let _ = app_handle.emit("token-state-changed", service.state());
                        }
                        Err(e) => {
                            log::error!("Periodic token refresh failed: {}", e);
                            // Don't set Failed state — the existing token might
                            // still be valid until it actually expires. Just log.
                        }
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window instead of closing when user clicks X
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_config,
            save_oauth_config,
            save_static_key_config,
            manual_refresh,
            log_out,
            has_credentials,
            start_refresh_loop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
