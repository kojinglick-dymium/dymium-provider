import Foundation

/// Service responsible for ensuring the opencode.json config has the dymium provider configured
final class OpenCodeConfigService {
    static let shared = OpenCodeConfigService()
    
    private let configPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".config/opencode/opencode.json")
    
    private let tokenPath = AppConfig.tokenPath.path
    
    /// The LLM endpoint URL from app config
    private var llmEndpoint: String {
        AppConfig.load().llmEndpoint
    }
    
    private init() {}
    
    /// Ensure the dymium provider and plugin are configured in opencode.json
    func ensureDymiumProvider() throws {
        // First, ensure the plugin exists
        try OpenCodePluginService.shared.ensurePlugin()
        
        // Ensure the config directory exists
        let configDir = configPath.deletingLastPathComponent()
        try FileManager.default.createDirectory(
            at: configDir,
            withIntermediateDirectories: true,
            attributes: nil
        )
        
        // Read existing config or start fresh
        var config: [String: Any]
        if FileManager.default.fileExists(atPath: configPath.path),
           let data = try? Data(contentsOf: configPath),
           let existingConfig = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            config = existingConfig
        } else {
            config = [
                "$schema": "https://opencode.ai/config.json"
            ]
        }
        
        var configChanged = false
        
        // Get or create the provider section
        var providers = config["provider"] as? [String: Any] ?? [:]
        
        // Add/update the dymium provider configuration
        if providers["dymium"] == nil {
            // Using @ai-sdk/openai-compatible since Dymium exposes an OpenAI-compatible API
            // Following the official OpenCode provider schema from anomalyco/opencode
            let dymiumProvider: [String: Any] = [
                "npm": "@ai-sdk/openai-compatible",
                "name": "Dymium",
                "api": llmEndpoint,  // The API endpoint URL
                "options": [
                    "baseURL": llmEndpoint
                ],
                "models": [
                    "claude-opus-4-5": [
                        "name": "Claude Opus 4.5 (via Dymium)",
                        "tool_call": true,
                        "temperature": true,
                        "attachment": true,
                        "reasoning": true,
                        "limit": [
                            "context": 200000,
                            "output": 16384
                        ]
                    ],
                    "claude-sonnet-4": [
                        "name": "Claude Sonnet 4 (via Dymium)",
                        "tool_call": true,
                        "temperature": true,
                        "attachment": true,
                        "reasoning": false,
                        "limit": [
                            "context": 200000,
                            "output": 16384
                        ]
                    ]
                ]
            ]
            
            providers["dymium"] = dymiumProvider
            config["provider"] = providers
            configChanged = true
            print("✅ Added dymium provider to opencode.json")
        } else {
            print("ℹ️  Dymium provider already configured in opencode.json")
        }
        
        // Ensure our plugin is registered
        configChanged = ensurePluginRegistered(in: &config) || configChanged
        
        // Write back the config if anything changed
        if configChanged {
            let jsonData = try JSONSerialization.data(
                withJSONObject: config,
                options: [.prettyPrinted, .sortedKeys]
            )
            try jsonData.write(to: configPath, options: .atomic)
            print("✅ Updated \(configPath.path)")
        }
        
        // Also update the auth.json with the token
        try updateAuthJson()
    }
    
    /// Ensure our dymium auth plugin is registered in the config
    /// Returns true if the config was modified
    private func ensurePluginRegistered(in config: inout [String: Any]) -> Bool {
        let pluginUrl = OpenCodePluginService.shared.pluginUrl
        var plugins = config["plugin"] as? [String] ?? []
        
        // Check if our plugin is already registered
        if plugins.contains(where: { $0.contains("dymium-opencode-plugin") || $0 == pluginUrl }) {
            print("ℹ️  Dymium auth plugin already registered")
            return false
        }
        
        // Add our plugin
        plugins.append(pluginUrl)
        config["plugin"] = plugins
        print("✅ Registered dymium auth plugin: \(pluginUrl)")
        return true
    }
    
    /// Update the auth.json file with the current token and GhostLLM app
    private func updateAuthJson() throws {
        let authPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/share/opencode/auth.json")
        
        // Ensure directory exists
        let authDir = authPath.deletingLastPathComponent()
        try FileManager.default.createDirectory(
            at: authDir,
            withIntermediateDirectories: true,
            attributes: nil
        )
        
        // Read existing auth or start fresh
        var auth: [String: Any]
        if FileManager.default.fileExists(atPath: authPath.path),
           let data = try? Data(contentsOf: authPath),
           let existingAuth = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            auth = existingAuth
        } else {
            auth = [:]
        }
        
        // Read the current token
        guard let token = TokenWriter.shared.readToken() else {
            print("No token available to write to auth.json")
            return
        }
        
        // Load config to get ghostllmApp
        let config = AppConfig.load()
        
        // Add/update dymium entry with proper format
        // Include the GhostLLM app name for X-GhostLLM-App header
        var dymiumAuth: [String: Any] = [
            "type": "api",
            "key": token
        ]
        
        // Add ghostllmApp if configured (required for OIDC/JWT auth)
        if let ghostllmApp = config.ghostllmApp, !ghostllmApp.isEmpty {
            dymiumAuth["app"] = ghostllmApp
            print("Including GhostLLM app in auth.json: \(ghostllmApp)")
        }
        
        auth["dymium"] = dymiumAuth
        
        // Write back
        let jsonData = try JSONSerialization.data(
            withJSONObject: auth,
            options: [.prettyPrinted, .sortedKeys]
        )
        try jsonData.write(to: authPath, options: .atomic)
        
        print("Updated dymium token in \(authPath.path)")
    }
    
    /// Force update just the auth token (called on every refresh)
    func updateToken() {
        do {
            try updateAuthJson()
        } catch {
            print("Failed to update auth.json: \(error)")
        }
    }
}
