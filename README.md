# DymiumProvider

A macOS menu bar application for seamless Keycloak authentication with Dymium/GhostLLM infrastructure.

## Overview

**DymiumProvider** is a Swift macOS menu bar app that:
- Authenticates with Keycloak using OAuth2 password/refresh token grants
- Automatically refreshes access tokens before expiry
- Syncs available models from the LLM endpoint to OpenCode config
- Manages the [dymium-auth-plugin](https://github.com/dymium-io/dymium-auth-plugin) installation and configuration

## Related Projects

- [dymium-auth-plugin](https://github.com/dymium-io/dymium-auth-plugin) - OpenCode plugin for HTTP/1.1 port-forward compatible requests

## Problem Solved

When using OpenCode with a custom LLM provider behind Keycloak authentication:

1. **Token Expiry**: Keycloak access tokens expire (typically 5 minutes). OpenCode caches the SDK at startup, so expired tokens cause auth failures.

2. **Port-Forward Compatibility**: When accessing services through `kubectl port-forward`, HTTP/2 and certain connection patterns can crash the tunnel with "connection reset by peer" errors.

3. **Istio Gateway Routing**: When port-forwarding to an Istio Gateway, the Host header must match the VirtualService configuration for proper routing.

## Architecture

```
┌─────────────────────┐     ┌──────────────────────┐
│   DymiumProvider    │     │      OpenCode        │
│   (Menu Bar App)    │     │                      │
├─────────────────────┤     ├──────────────────────┤
│ • Keycloak Auth     │────▶│ ~/.local/share/      │
│ • Token Refresh     │     │   opencode/auth.json │
│ • Model Sync        │     │                      │
│ • Plugin Management │     │ ~/.config/opencode/  │
└─────────────────────┘     │   opencode.json      │
                            └──────────┬───────────┘
                                       │
                            ┌──────────▼───────────┐
                            │  dymium-auth-plugin  │
                            ├──────────────────────┤
                            │ • Reads fresh token  │
                            │ • HTTP/1.1 requests  │
                            │ • Istio Host headers │
                            └──────────┬───────────┘
                                       │
                            ┌──────────▼───────────┐
                            │   kubectl port-fwd   │
                            │   → Istio Gateway    │
                            │   → GhostLLM Backend │
                            └──────────────────────┘
```

## Installation

### Prerequisites

- macOS 13.0+
- Swift 5.9+
- OpenCode CLI installed
- Keycloak instance with configured client credentials
- kubectl port-forward to Istio Gateway (if using remote cluster)

### Build DymiumProvider

```bash
cd DymiumProvider
swift build -c release
# Binary at .build/release/DymiumProvider
```

### Configuration

Create `~/.dymium/config.json`:

```json
{
  "keycloakURL": "https://keycloak.example.com:9173",
  "realm": "dymium",
  "clientId": "dymium",
  "username": "your-username@example.com",
  "refreshIntervalSeconds": 60,
  "llmEndpoint": "http://your-llm-endpoint:3000/v1",
  "clientSecret": "your-client-secret",
  "password": "your-password"
}
```

### Usage

1. Run DymiumProvider (it will appear in the menu bar)
2. The app will:
   - Authenticate with Keycloak
   - Write tokens to `~/.dymium/token` and `~/.local/share/opencode/auth.json`
   - Install the OpenCode plugin at `~/.local/share/dymium-opencode-plugin/`
   - Register the plugin in `~/.config/opencode/opencode.json`
   - Sync available models from the LLM endpoint

3. Start OpenCode - it will use the "dymium" provider with automatic token refresh

## OpenCode Configuration

The app automatically configures OpenCode with the dymium provider:

```json
{
  "provider": {
    "dymium": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Dymium",
      "api": "http://your-llm-endpoint:3000/v1",
      "models": {
        "claude-opus-4-5": {
          "name": "Claude Opus 4.5 (GhostLLM)",
          "tool_call": true,
          "reasoning": true,
          ...
        }
      }
    }
  },
  "plugin": [
    "file:///Users/you/.local/share/dymium-opencode-plugin"
  ]
}
```

## Technical Details

### Token Flow

1. DymiumProvider authenticates with Keycloak (password grant or refresh token)
2. Access token written to `~/.local/share/opencode/auth.json`
3. Plugin reads token fresh from auth.json on every API request
4. Token automatically refreshed every 60 seconds (configurable)

### Model Sync

On every token refresh, the app calls `/v1/models` to:
- Fetch available models from the LLM endpoint
- Automatically add new models to opencode.json with sensible defaults
- Remove models that are no longer available
- Preserve existing model configurations

## Development

### Project Structure

```
DymiumProvider/
├── Package.swift
└── DymiumProvider/
    ├── App.swift
    ├── Config/
    │   └── AppConfig.swift
    ├── Services/
    │   ├── TokenService.swift          # Keycloak auth
    │   ├── TokenWriter.swift           # Token file management
    │   ├── KeychainService.swift       # Secure storage
    │   ├── OpenCodeConfigService.swift # opencode.json management
    │   ├── OpenCodePluginService.swift # Plugin generation
    │   └── ModelSyncService.swift      # Model sync from LLM endpoint
    ├── Model/
    │   └── TokenState.swift
    └── Views/
        ├── PopoverView.swift
        └── GhostShape.swift
```

### Building

```bash
swift build           # Debug build
swift build -c release  # Release build
```

## License

MIT
