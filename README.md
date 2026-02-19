# Dymium Provider

Cross-platform authentication manager for [GhostLLM](https://dymium.io) that integrates with [OpenCode](https://opencode.ai).

![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## Overview

**Dymium Provider** is a system tray application that manages authentication tokens for GhostLLM. It runs silently in the background, automatically refreshing your OAuth tokens or managing static API keys, so you can use AI coding assistants like OpenCode without worrying about authentication.

### Key Features

- **System Tray Application** - Runs silently in the background
- **OAuth (Keycloak)** - Automatic token refresh with password grant and refresh token support
- **Static API Key** - Simple mode for environments with static authentication
- **OpenCode Integration** - Automatically configures OpenCode with the `dymium` provider
- **Cross-Platform** - Works on macOS, Linux, and Windows

## System Requirements

| Dependency | Minimum Version | Notes |
|------------|----------------|-------|
| [OpenCode](https://opencode.ai) | **v1.2.0+** | Older versions have a bug where `@ai-sdk/openai-compatible` strips the path component from `baseURL`, breaking OIDC app-path routing. |
| [dymium-auth-plugin](https://www.npmjs.com/package/dymium-auth-plugin) | 1.2.0 | Listed in `opencode.json` plugins; installed automatically by OpenCode. |
| GhostLLM endpoint | Reachable via HTTP | Typically tunneled through `ssh -L 9090:localhost:9090` to a `kubectl port-forward`. |

### Verifying OpenCode version

```bash
opencode -v   # must be >= 1.2.0
opencode update   # upgrade if needed
```

## Quick Start

See the full documentation in [`DymiumProvider/README.md`](./DymiumProvider/README.md).

### Download

Download pre-built binaries from the [Releases page](https://github.com/kojinglick-dymium/dymium-provider/releases):

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | `Dymium.Provider_x.x.x_aarch64.dmg` |
| macOS (Intel) | `Dymium.Provider_x.x.x_x64.dmg` |
| Linux (Debian/Ubuntu) | `dymium-provider_x.x.x_amd64.deb` |
| Linux (AppImage) | `dymium-provider_x.x.x_amd64.AppImage` |
| Windows | `Dymium.Provider_x.x.x_x64-setup.exe` or `.msi` |

### macOS Note

The app is not yet code-signed. After installation, run:

```bash
xattr -cr "/Applications/Dymium Provider.app"
```

## Related Projects

- [dymium-auth-plugin](https://github.com/dymium-io/dymium-auth-plugin) - OpenCode plugin for HTTP/1.1 port-forward compatible requests

## Project Structure

```
dymium-provider/
├── DymiumProvider/          # Main Tauri v2 app (cross-platform)
│   ├── src/                 # React/TypeScript frontend
│   ├── src-tauri/           # Rust backend
│   └── README.md            # Full documentation
├── DymiumProviderMacOS/     # Legacy SwiftUI app (archived)
└── .github/workflows/       # CI/CD for releases
```

## License

MIT
