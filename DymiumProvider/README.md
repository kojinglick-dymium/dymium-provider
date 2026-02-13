# Dymium Provider

Cross-platform authentication manager for [GhostLLM](https://dymium.io) that integrates with [OpenCode](https://opencode.ai).

![Dymium Provider](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## What is Dymium Provider?

Dymium Provider is a system tray application that manages authentication tokens for GhostLLM. It runs silently in the background, automatically refreshing your OAuth tokens or managing static API keys, so you can use AI coding assistants like OpenCode without worrying about authentication.

### Key Features

- **System Tray Application** - Runs silently in the background
- **OAuth (Keycloak)** - Automatic token refresh with password grant and refresh token support  
- **Static API Key** - Simple mode for environments with static authentication
- **OpenCode Integration** - Automatically configures OpenCode with the `dymium` provider
- **Cross-Platform** - Works on macOS, Linux, and Windows

---

## Installation

### Download Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/kojinglick-dymium/dymium-provider/releases):

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | `Dymium Provider_x.x.x_aarch64.dmg` |
| macOS (Intel) | `Dymium Provider_x.x.x_x64.dmg` |
| Linux (Debian/Ubuntu) | `dymium-provider_x.x.x_amd64.deb` |
| Linux (Universal) | `dymium-provider_x.x.x_amd64.AppImage` |
| Windows | `Dymium Provider_x.x.x_x64-setup.exe` or `.msi` |

### Linux Installation

```bash
# Debian/Ubuntu - install the .deb package
sudo dpkg -i dymium-provider_*.deb

# Or use the AppImage (no installation required)
chmod +x dymium-provider_*.AppImage
./dymium-provider_*.AppImage
```

### macOS Installation

1. Open the `.dmg` file
2. Drag "Dymium Provider" to your Applications folder
3. **Important: Clear the quarantine attribute** (required for unsigned apps):
   ```bash
   xattr -cr "/Applications/Dymium Provider.app"
   ```
4. Launch the app - it will appear in your menu bar

> **Note:** The app is not yet code-signed with an Apple Developer certificate. Without step 3, macOS will show "Dymium Provider is damaged and can't be opened." This is a Gatekeeper protection for unsigned apps, not actual damage.

---

## Prerequisites: Install OpenCode First

**Important:** Dymium Provider integrates with OpenCode by automatically creating configuration files. For this to work correctly, **OpenCode must be installed first**.

### Install OpenCode

```bash
# Using npm
npm install -g opencode

# Or using the install script
curl -fsSL https://opencode.ai/install.sh | bash
```

After installing OpenCode, run it once to create the config directories:

```bash
opencode --version
```

This ensures the following directories exist:
- `~/.config/opencode/` (config directory)
- `~/.local/share/opencode/` (data directory)

---

## Quick Start

### 1. Launch Dymium Provider

After installation, launch the app. It will appear as an icon in your system tray (menu bar on macOS).

### 2. Open Setup

Click the tray icon and select **"Setup..."** to open the configuration window.

### 3. Choose Authentication Mode

#### Option A: OAuth (Keycloak)

For organizations using Keycloak for identity management:

| Field | Description | Example |
|-------|-------------|---------|
| LLM Endpoint | Your GhostLLM API endpoint | `http://ghostllm.company.com:3000/v1` |
| Keycloak URL | Your Keycloak server | `https://auth.company.com:9173` |
| Username | Your Keycloak username | `user@company.com` |
| GhostLLM App | The application name in GhostLLM | `my-coding-assistant` |
| Client Secret | OAuth client secret from Keycloak | `abc123...` |
| Password | Your Keycloak password | `********` |

Advanced settings (usually defaults are fine):
- **Realm**: `dymium` (default)
- **Client ID**: `dymium` (default)

#### Option B: Static API Key

For simpler setups with static API keys:

| Field | Description | Example |
|-------|-------------|---------|
| LLM Endpoint | Your GhostLLM API endpoint | `http://ghostllm.company.com:3000/v1` |
| Static API Key | Your GhostLLM API key | `sk-abc123...` |

### 4. Save & Connect

Click **"Save & Connect"**. The app will:
1. Authenticate with your credentials
2. Write the token to `~/.dymium/token`
3. Configure OpenCode automatically (see below)
4. Start refreshing tokens in the background (OAuth mode)

### 5. Verify in OpenCode

Open a new terminal and run:

```bash
opencode
```

You should see `dymium` as an available provider. Select a model like `dymium/claude-opus-4-5` to start coding!

---

## How OpenCode Integration Works

When you save your configuration, Dymium Provider automatically:

### 1. Creates the OpenCode Provider Config

Adds the `dymium` provider to `~/.config/opencode/opencode.json`:

```json
{
  "provider": {
    "dymium": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Dymium",
      "api": "http://your-endpoint:3000/v1",
      "models": {
        "claude-opus-4-5": { ... },
        "claude-sonnet-4": { ... }
      }
    }
  },
  "plugin": [
    "file:///home/user/.local/share/dymium-opencode-plugin"
  ]
}
```

### 2. Installs an Auth Plugin

Creates a TypeScript plugin at `~/.local/share/dymium-opencode-plugin/` that:
- Reads the fresh token from `~/.local/share/opencode/auth.json` on every request
- Injects the `Authorization: Bearer <token>` header
- Uses HTTP/1.1 for compatibility with kubectl port-forward setups

### 3. Updates Auth Credentials

Writes the current token to `~/.local/share/opencode/auth.json`:

```json
{
  "dymium": {
    "type": "api",
    "key": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9..."
  }
}
```

This file is updated automatically whenever the token refreshes.

---

## Configuration Files

| File | Purpose |
|------|---------|
| `~/.dymium/config.json` | Dymium Provider settings and credentials |
| `~/.dymium/token` | Current access token (plain text) |
| `~/.config/opencode/opencode.json` | OpenCode configuration (auto-updated) |
| `~/.local/share/opencode/auth.json` | OpenCode auth tokens (auto-updated) |
| `~/.local/share/dymium-opencode-plugin/` | OpenCode auth plugin (auto-created) |

### Example `~/.dymium/config.json`

```json
{
  "authMode": "oauth",
  "llmEndpoint": "http://ghostllm.company.com:3000/v1",
  "keycloakUrl": "https://auth.company.com:9173",
  "realm": "dymium",
  "clientId": "dymium",
  "username": "user@company.com",
  "ghostllmApp": "my-app",
  "refreshIntervalSeconds": 60,
  "clientSecret": "...",
  "password": "...",
  "refreshToken": "..."
}
```

---

## Tray Menu Options

| Menu Item | Description |
|-----------|-------------|
| **Status** | Shows current authentication state |
| **Refresh Now** | Manually trigger a token refresh |
| **Setup...** | Open the configuration window |
| **Quit** | Exit the application |

---

## Troubleshooting

### macOS: "Dymium Provider is damaged and can't be opened"

This error occurs because the app is not code-signed with an Apple Developer certificate. Fix it by removing the quarantine attribute:

```bash
xattr -cr "/Applications/Dymium Provider.app"
```

Then launch the app normally.

### "dymium" provider not appearing in OpenCode

1. Ensure OpenCode is installed and has been run at least once
2. Check that `~/.config/opencode/opencode.json` contains the `dymium` provider
3. Restart OpenCode after Dymium Provider configures it

### Token refresh failing

1. Check the tray icon status - it should show "Authenticated"
2. Verify your Keycloak credentials are correct
3. Ensure the Keycloak server is reachable
4. Check `~/.dymium/config.json` for correct URLs

### Self-signed certificate issues

Dymium Provider accepts self-signed certificates by default for local/development Keycloak instances.

### Linux: Tray icon not visible

Install the AppIndicator library:

```bash
# Ubuntu/Debian
sudo apt install libappindicator3-1

# Fedora
sudo dnf install libappindicator-gtk3
```

---

## Building from Source

### Prerequisites

- Node.js 20+
- Rust 1.70+
- Platform-specific dependencies

### macOS

```bash
cd DymiumProvider
npm install
npm run tauri build
```

### Linux

```bash
# Install dependencies first
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

cd DymiumProvider
npm install
npm run tauri build
```

### Windows

```bash
cd DymiumProvider
npm install
npm run tauri build
```

### Development Mode

```bash
cd DymiumProvider
npm install
npm run tauri dev
```

---

## Architecture

```
DymiumProvider/
├── src/                      # React frontend
│   ├── App.tsx              # Setup window UI
│   └── App.css              # Dymium brand styles
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs           # Main app, tray menu, Tauri commands
│   │   └── services/
│   │       ├── config.rs    # Configuration management
│   │       ├── token.rs     # OAuth token management
│   │       ├── keystore.rs  # Credential storage (keyring)
│   │       └── opencode.rs  # OpenCode integration
│   └── plugin/
│       └── index.ts         # OpenCode auth plugin (embedded)
└── package.json
```

---

## Legacy macOS App

The original native SwiftUI macOS app is archived at `../DymiumProviderMacOS/` for reference.

---

## License

MIT
