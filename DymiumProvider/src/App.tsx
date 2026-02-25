import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

// Types matching Rust backend
type AuthMode = "OAuth" | "StaticKey";

interface TokenState {
  type: "idle" | "authenticating" | "verifying" | "authenticated" | "failed";
  token?: string;
  expiresAt?: string;
  error?: string;
}

interface AppConfig {
  authMode: AuthMode;
  llmEndpoint: string;
  keycloakUrl: string;
  clientId: string;
  username: string;
  realm: string;
  refreshIntervalSeconds: number;
  ghostllmApp?: string;
  clientSecret?: string;
  password?: string;
  staticApiKey?: string;
}

function statusLabelFromError(error?: string): string {
  const normalized = (error || "").toLowerCase();
  if (
    normalized.includes("401") ||
    normalized.includes("unauthorized") ||
    normalized.includes("invalid api key") ||
    normalized.includes("invalid oidc token")
  ) {
    return "Unauthorized";
  }
  if (normalized.includes("timed out")) return "Endpoint timeout";
  if (normalized.includes("cannot reach llm endpoint")) return "Endpoint unreachable";
  if (normalized.includes("failed to update opencode config")) return "OpenCode config error";
  return "Failed";
}

// Ghost icon component
function GhostIcon({ state }: { state: TokenState }) {
  const stateClass = state.type === "authenticated" ? "authenticated" 
    : state.type === "authenticating" || state.type === "verifying" ? "authenticating"
    : state.type === "failed" ? "failed" 
    : "idle";

  return (
    <div className="ghost-icon">
      <svg viewBox="0 0 24 32" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="gradient" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="#904dff" />
            <stop offset="100%" stopColor="#4369ff" />
          </linearGradient>
        </defs>
        <path
          className={`ghost-outline ${stateClass}`}
          d="M2 28 Q4 26 6 28 Q8 26 10 28 Q12 26 14 28 Q16 26 18 28 Q20 26 22 28 
             L22 12 A10 10 0 0 0 2 12 Z"
        />
      </svg>
    </div>
  );
}

function App() {
  // State
  const [tokenState, setTokenState] = useState<TokenState>({ type: "idle" });
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [authMode, setAuthMode] = useState<AuthMode>("OAuth");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  // Form fields - OAuth
  const [llmEndpoint, setLlmEndpoint] = useState("");
  const [keycloakUrl, setKeycloakUrl] = useState("");
  const [username, setUsername] = useState("");
  const [ghostllmApp, setGhostllmApp] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [password, setPassword] = useState("");
  const [realm, setRealm] = useState("dymium");
  const [clientId, setClientId] = useState("dymium");

  // Form fields - Static Key
  const [staticApiKey, setStaticApiKey] = useState("");

  // Load initial state
  useEffect(() => {
    loadState();
    
    // Listen for state changes from backend
    const unlisten = listen<TokenState>("token-state-changed", (event) => {
      setTokenState(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  async function loadState() {
    try {
      const [state, cfg] = await Promise.all([
        invoke<TokenState>("get_state"),
        invoke<AppConfig>("get_config"),
      ]);
      setTokenState(state);
      setConfig(cfg);
      
      // Populate form from config
      setAuthMode(cfg.authMode);
      setLlmEndpoint(cfg.llmEndpoint || "");
      setKeycloakUrl(cfg.keycloakUrl || "");
      setUsername(cfg.username || "");
      setGhostllmApp(cfg.ghostllmApp || "");
      setRealm(cfg.realm || "dymium");
      setClientId(cfg.clientId || "dymium");
      setStaticApiKey(cfg.staticApiKey || "");
      // Don't populate secrets for security
    } catch (e) {
      console.error("Failed to load state:", e);
    }
  }

  async function handleSave() {
    setIsSaving(true);
    setError(null);

    try {
      if (authMode === "OAuth") {
        await invoke("save_oauth_config", {
          keycloakUrl,
          realm,
          clientId,
          username,
          llmEndpoint,
          ghostllmApp: ghostllmApp || null,
          clientSecret,
          password,
        });
      } else {
        await invoke("save_static_key_config", {
          llmEndpoint,
          staticApiKey,
          ghostllmApp: ghostllmApp || null,
        });
      }

      // Start refresh loop
      await invoke("start_refresh_loop");
      
      // Hide window after successful save
      await getCurrentWindow().hide();
    } catch (e) {
      setError(String(e));
    } finally {
      await loadState();
      setIsSaving(false);
    }
  }

  async function handleRefresh() {
    try {
      await invoke("manual_refresh");
      await loadState(); // Reload state after refresh
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleLogOut() {
    try {
      await invoke("log_out");
      setTokenState({ type: "idle" });
      setClientSecret("");
      setPassword("");
      setStaticApiKey("");
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleCancel() {
    await getCurrentWindow().hide();
  }

  const isFormValid = authMode === "OAuth"
    ? llmEndpoint && keycloakUrl && username && clientSecret && password && realm && clientId && ghostllmApp
    : llmEndpoint && staticApiKey;  // ghostllmApp is optional for static key (legacy lookup)

  const hasCredentials = config && (
    authMode === "OAuth"
      ? config.clientSecret && config.password
      : config.staticApiKey
  );

  return (
    <div className="app">
      {/* Header */}
      <div className="header">
        <GhostIcon state={tokenState} />
        <div className="header-text">
          <h1>Dymium Setup</h1>
          <p>Configure your GhostLLM connection</p>
        </div>
      </div>

      <div className="divider" />

      {/* Auth Mode Picker */}
      <div className="auth-mode-picker">
        <button
          className={`auth-mode-btn ${authMode === "OAuth" ? "active" : ""}`}
          onClick={() => setAuthMode("OAuth")}
        >
          OAuth (Keycloak)
        </button>
        <button
          className={`auth-mode-btn ${authMode === "StaticKey" ? "active" : ""}`}
          onClick={() => setAuthMode("StaticKey")}
        >
          Static API Key
        </button>
      </div>

      {/* Form */}
      <div className="form-scroll">
        {/* Common: LLM Endpoint */}
        <div className="form-section">
          <h3>Endpoint</h3>
          <div className="field">
            <label>LLM Endpoint</label>
            <input
              type="text"
              value={llmEndpoint}
              onChange={(e) => setLlmEndpoint(e.target.value)}
              placeholder="http://spoofcorp.llm.dymium.home:9090/v1"
            />
          </div>
        </div>

        {authMode === "OAuth" ? (
          <>
            {/* OAuth: Connection */}
            <div className="form-section">
              <h3>Keycloak Connection</h3>
              <div className="field">
                <label>Keycloak URL</label>
                <input
                  type="text"
                  value={keycloakUrl}
                  onChange={(e) => setKeycloakUrl(e.target.value)}
                  placeholder="https://192.168.50.100:9173"
                />
              </div>
              <div className="field">
                <label>Username</label>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="user@example.com"
                />
              </div>
              <div className="field">
                <label>GhostLLM App</label>
                <input
                  type="text"
                  value={ghostllmApp}
                  onChange={(e) => setGhostllmApp(e.target.value)}
                  placeholder="your-ghostllm-app-name"
                />
              </div>
            </div>

            {/* OAuth: Credentials */}
            <div className="form-section">
              <h3>Credentials</h3>
              <div className="field">
                <label>Client Secret</label>
                <input
                  type="password"
                  value={clientSecret}
                  onChange={(e) => setClientSecret(e.target.value)}
                  placeholder="Client secret from Keycloak"
                />
              </div>
              <div className="field">
                <label>Password</label>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Your password"
                />
              </div>
            </div>

            {/* Advanced Settings */}
            <div
              className="advanced-toggle"
              onClick={() => setShowAdvanced(!showAdvanced)}
            >
              <span>{showAdvanced ? "▼" : "▶"}</span>
              <span>Advanced Settings</span>
            </div>
            <div className={`advanced-content ${showAdvanced ? "open" : ""}`}>
              <div className="form-section">
                <div className="field">
                  <label>Realm</label>
                  <input
                    type="text"
                    value={realm}
                    onChange={(e) => setRealm(e.target.value)}
                    placeholder="dymium"
                  />
                </div>
                <div className="field">
                  <label>Client ID</label>
                  <input
                    type="text"
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    placeholder="dymium"
                  />
                </div>
              </div>
            </div>

            <p className="info-text">
              OAuth credentials are stored securely. Tokens refresh automatically.
            </p>
          </>
        ) : (
          <>
            {/* Static Key */}
            <div className="form-section">
              <h3>API Key</h3>
              <div className="field">
                <label>Static API Key</label>
                <input
                  type="password"
                  value={staticApiKey}
                  onChange={(e) => setStaticApiKey(e.target.value)}
                  placeholder="Your GhostLLM API key"
                />
              </div>
              <div className="field">
                <label>GhostLLM App</label>
                <input
                  type="text"
                  value={ghostllmApp}
                  onChange={(e) => setGhostllmApp(e.target.value)}
                  placeholder="Application name (e.g., static_testing)"
                />
              </div>
            </div>

            <p className="info-text">
              Static API key is stored in ~/.dymium/config.json. No automatic refresh needed.
            </p>
          </>
        )}

        {/* Status display */}
        {tokenState.type === "idle" && (
          <div className="status-section">
            <div className="status-row">
              <span className="label">Status:</span>
              <span className="value">Not configured</span>
            </div>
          </div>
        )}

        {tokenState.type === "authenticating" && (
          <div className="status-section">
            <div className="status-row">
              <span className="label">Status:</span>
              <span className="value warning">Connecting...</span>
            </div>
          </div>
        )}

        {tokenState.type === "verifying" && (
          <div className="status-section">
            <div className="status-row">
              <span className="label">Status:</span>
              <span className="value warning">Verifying endpoint...</span>
            </div>
          </div>
        )}

        {tokenState.type === "authenticated" && (
          <div className="status-section">
            <div className="status-row">
              <span className="label">Status:</span>
              <span className="value success">Connected</span>
            </div>
            {tokenState.expiresAt && (
              <div className="status-row">
                <span className="label">Expires:</span>
                <span className="value">
                  {new Date(tokenState.expiresAt).toLocaleTimeString()}
                </span>
              </div>
            )}
          </div>
        )}

        {tokenState.type === "failed" && (
          <div className="status-section">
            <div className="status-row">
              <span className="label">Status:</span>
              <span className="value error">{statusLabelFromError(tokenState.error)}</span>
            </div>
            <div className="status-row">
              <span className="value error">{tokenState.error}</span>
            </div>
          </div>
        )}
      </div>

      {/* Error message */}
      {error && <div className="error-message">{error}</div>}

      <div className="divider" />

      {/* Buttons */}
      <div className="button-row">
        <button className="btn btn-secondary" onClick={handleCancel}>
          Cancel
        </button>
        <div className="spacer" />
        {hasCredentials && (
          <>
            <button className="btn btn-secondary" onClick={handleRefresh}>
              Refresh
            </button>
            <button className="btn btn-danger" onClick={handleLogOut}>
              Log Out
            </button>
          </>
        )}
        <button
          className="btn btn-primary"
          onClick={handleSave}
          disabled={!isFormValid || isSaving}
        >
          {isSaving ? "Saving..." : "Save & Connect"}
        </button>
      </div>
    </div>
  );
}

export default App;
