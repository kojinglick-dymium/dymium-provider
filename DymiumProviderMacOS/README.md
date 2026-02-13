# Dymium Provider (Legacy macOS - SwiftUI)

> **Note:** This is the legacy native SwiftUI macOS app. It has been superseded by the cross-platform Tauri v2 app at `../DymiumProvider/`. Use the Tauri version for active development and new features.

## About

This was the original macOS-only implementation of Dymium Provider using SwiftUI. It provided:

- Native macOS menu bar application
- OAuth (Keycloak) authentication with token refresh
- Static API key support
- OpenCode integration

## Building (macOS only)

### Prerequisites

- macOS 13.0+
- Xcode 15+ or Swift 5.9+

### Build with Swift Package Manager

```bash
cd DymiumProviderMacOS
swift build -c release
```

The binary will be at `.build/release/DymiumProvider`.

### Run

```bash
.build/release/DymiumProvider
```

## Project Structure

```
DymiumProviderMacOS/
├── Package.swift              # Swift package manifest
└── DymiumProvider/
    ├── App.swift              # Main app entry point
    ├── Info.plist             # App metadata
    ├── DymiumProvider.entitlements
    ├── Config/                # Configuration management
    ├── Model/                 # Data models
    ├── Services/              # Token, keychain, OpenCode services
    └── Views/                 # SwiftUI views
```

## Why Deprecated?

The SwiftUI version was replaced with Tauri v2 to support:

- **Cross-platform**: macOS, Linux, and Windows
- **Easier distribution**: No Apple Developer certificate required for basic functionality
- **Web technologies**: React/TypeScript frontend for faster UI iteration
- **Rust backend**: Same performance as Swift with better cross-platform support

## Migration

For new installations, use the Tauri version at `../DymiumProvider/`. Configuration files are compatible between both versions (`~/.dymium/config.json`).
