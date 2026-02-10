import Foundation

/// Authentication mode for GhostLLM
enum AuthMode: String, Codable, CaseIterable {
    case oauth = "oauth"
    case staticKey = "staticKey"
    
    var displayName: String {
        switch self {
        case .oauth: return "OAuth (Keycloak)"
        case .staticKey: return "Static API Key"
        }
    }
}

/// Configuration for the Dymium Provider app
/// Loaded from ~/.dymium/config.json or uses defaults
struct AppConfig: Codable {
    /// Authentication mode: OAuth (Keycloak) or Static API Key
    var authMode: AuthMode
    
    /// LLM endpoint URL (required for both modes)
    var llmEndpoint: String
    
    // --- OAuth mode fields ---
    var keycloakURL: String
    var clientId: String
    var username: String
    var realm: String
    var refreshIntervalSeconds: Int
    
    /// The GhostLLM application name or ID (required for OIDC/JWT auth)
    /// This is sent as the X-GhostLLM-App header to identify the app configuration
    var ghostllmApp: String?
    
    // OAuth credentials
    var clientSecret: String?
    var password: String?
    var refreshToken: String?
    
    // --- Static API Key mode fields ---
    /// Static API key for direct authentication (no token refresh needed)
    var staticApiKey: String?
    
    static let configDirectory = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".dymium")
    
    static let configPath = configDirectory.appendingPathComponent("config.json")
    static let tokenPath = configDirectory.appendingPathComponent("token")
    
    // MARK: - Custom Decoding for Backward Compatibility
    
    enum CodingKeys: String, CodingKey {
        case authMode, llmEndpoint, keycloakURL, clientId, username, realm
        case refreshIntervalSeconds, ghostllmApp, clientSecret, password
        case refreshToken, staticApiKey
    }
    
    /// Custom decoder to handle old configs without authMode field
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        
        // Decode all fields with defaults for optional/missing ones
        self.llmEndpoint = try container.decodeIfPresent(String.self, forKey: .llmEndpoint) ?? ""
        self.keycloakURL = try container.decodeIfPresent(String.self, forKey: .keycloakURL) ?? ""
        self.clientId = try container.decodeIfPresent(String.self, forKey: .clientId) ?? ""
        self.username = try container.decodeIfPresent(String.self, forKey: .username) ?? ""
        self.realm = try container.decodeIfPresent(String.self, forKey: .realm) ?? ""
        self.refreshIntervalSeconds = try container.decodeIfPresent(Int.self, forKey: .refreshIntervalSeconds) ?? 60
        self.ghostllmApp = try container.decodeIfPresent(String.self, forKey: .ghostllmApp)
        self.clientSecret = try container.decodeIfPresent(String.self, forKey: .clientSecret)
        self.password = try container.decodeIfPresent(String.self, forKey: .password)
        self.refreshToken = try container.decodeIfPresent(String.self, forKey: .refreshToken)
        self.staticApiKey = try container.decodeIfPresent(String.self, forKey: .staticApiKey)
        
        // Decode authMode with smart default:
        // - If authMode is explicitly set, use it
        // - If staticApiKey is present, infer staticKey mode
        // - Otherwise default to oauth
        if let mode = try container.decodeIfPresent(AuthMode.self, forKey: .authMode) {
            self.authMode = mode
        } else if let apiKey = self.staticApiKey, !apiKey.isEmpty {
            // Infer static key mode from presence of staticApiKey
            self.authMode = .staticKey
        } else {
            // Default to OAuth for backward compatibility
            self.authMode = .oauth
        }
    }
    
    /// Standard memberwise initializer
    init(
        authMode: AuthMode,
        llmEndpoint: String,
        keycloakURL: String,
        clientId: String,
        username: String,
        realm: String,
        refreshIntervalSeconds: Int,
        ghostllmApp: String?,
        clientSecret: String?,
        password: String?,
        refreshToken: String?,
        staticApiKey: String?
    ) {
        self.authMode = authMode
        self.llmEndpoint = llmEndpoint
        self.keycloakURL = keycloakURL
        self.clientId = clientId
        self.username = username
        self.realm = realm
        self.refreshIntervalSeconds = refreshIntervalSeconds
        self.ghostllmApp = ghostllmApp
        self.clientSecret = clientSecret
        self.password = password
        self.refreshToken = refreshToken
        self.staticApiKey = staticApiKey
    }
    
    /// Default configuration pointing to the local Keycloak instance
    static let `default` = AppConfig(
        authMode: .oauth,
        llmEndpoint: "http://spoofcorp.llm.dymium.home:3000/v1",
        keycloakURL: "https://192.168.50.100:9173",
        clientId: "dymium",
        username: "dev_mcp_admin@dymium.io",
        realm: "dymium",
        refreshIntervalSeconds: 60, // 1 minute - well within the 5-minute access token lifetime
        ghostllmApp: nil, // Must be set to your GhostLLM app name for OIDC auth
        clientSecret: nil,
        password: nil,
        refreshToken: nil,
        staticApiKey: nil
    )
    
    /// Load configuration from disk or return defaults
    static func load() -> AppConfig {
        do {
            let data = try Data(contentsOf: configPath)
            let config = try JSONDecoder().decode(AppConfig.self, from: data)
            return config
        } catch {
            print("Could not load config from \(configPath.path), using defaults: \(error)")
            return .default
        }
    }
    
    /// Save configuration to disk
    func save() throws {
        try FileManager.default.createDirectory(
            at: AppConfig.configDirectory,
            withIntermediateDirectories: true
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(self)
        try data.write(to: AppConfig.configPath)
    }
    
    /// The full token endpoint URL (OAuth mode only)
    var tokenEndpointURL: URL? {
        URL(string: "\(keycloakURL)/realms/\(realm)/protocol/openid-connect/token")
    }
    
    /// Whether using static API key authentication
    var isStaticKeyMode: Bool {
        authMode == .staticKey
    }
    
    /// Whether using OAuth authentication
    var isOAuthMode: Bool {
        authMode == .oauth
    }
}

/// Keys for storing secrets (used for credential storage interface)
enum CredentialKey: String {
    case clientSecret
    case password
    case refreshToken
}
