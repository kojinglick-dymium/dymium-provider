# Dymium Provider

Cross-platform GhostLLM authentication manager built with Tauri v2.

## Features

- **System Tray Application**: Runs silently in the background
- **OAuth (Keycloak)**: Automatic token refresh with password grant and refresh token support
- **Static API Key**: Simple mode for static authentication
- **OpenCode Integration**: Automatically configures OpenCode with the dymium provider
- **Cross-Platform**: Builds for macOS, Linux, and Windows

## Building

### Prerequisites

- Node.js 20+
- Rust 1.70+
- Platform-specific dependencies (see below)

### macOS

```bash
npm install
npm run tauri build
```

The built app will be at:
- `src-tauri/target/release/bundle/macos/Dymium Provider.app`
- `src-tauri/target/release/bundle/dmg/Dymium Provider_*.dmg`

### Linux

Install system dependencies first:

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

# Fedora
sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel

# Arch
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg
```

Then build:

```bash
npm install
npm run tauri build
```

The built app will be at:
- `src-tauri/target/release/bundle/deb/dymium-provider_*.deb`
- `src-tauri/target/release/bundle/appimage/dymium-provider_*.AppImage`

### Windows

```bash
npm install
npm run tauri build
```

The built app will be at:
- `src-tauri/target/release/bundle/msi/Dymium Provider_*.msi`
- `src-tauri/target/release/bundle/nsis/Dymium Provider_*.exe`

## Development

```bash
npm install
npm run tauri dev
```

## Configuration

Configuration is stored at `~/.dymium/config.json`:

```json
{
  "authMode": "oauth",
  "llmEndpoint": "http://your-llm-endpoint:3000/v1",
  "keycloakUrl": "https://your-keycloak:9173",
  "realm": "dymium",
  "clientId": "dymium",
  "username": "user@example.com",
  "ghostllmApp": "your-app-name"
}
```

Credentials are stored in the same file (or system keyring in future versions).

## Architecture

```
DymiumProvider/
├── src/                    # React frontend
│   ├── App.tsx            # Main setup window UI
│   └── App.css            # Dymium brand styles
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs         # Main app, tray menu, commands
│   │   └── services/
│   │       ├── config.rs  # Configuration loading/saving
│   │       ├── token.rs   # OAuth token management
│   │       ├── keystore.rs # Secure credential storage
│   │       └── opencode.rs # OpenCode config integration
│   └── plugin/
│       └── index.ts       # OpenCode auth plugin (embedded)
└── package.json
```

## Legacy macOS App

The original SwiftUI macOS app is archived at `../DymiumProviderMacOS/`.
