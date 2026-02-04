import Foundation

/// Configuration for the Dymium Provider app
/// Loaded from ~/.dymium/config.json or uses defaults
struct AppConfig: Codable {
    var keycloakURL: String
    var clientId: String
    var username: String
    var realm: String
    var refreshIntervalSeconds: Int
    var llmEndpoint: String
    
    // Credentials stored in config file (not using keychain)
    var clientSecret: String?
    var password: String?
    var refreshToken: String?
    
    static let configDirectory = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".dymium")
    
    static let configPath = configDirectory.appendingPathComponent("config.json")
    static let tokenPath = configDirectory.appendingPathComponent("token")
    
    /// Default configuration pointing to the local Keycloak instance
    static let `default` = AppConfig(
        keycloakURL: "https://192.168.50.100:9173",
        clientId: "dymium",
        username: "dev_mcp_admin@dymium.io",
        realm: "dymium",
        refreshIntervalSeconds: 60, // 1 minute - well within the 5-minute access token lifetime
        llmEndpoint: "http://spoofcorp.llm.dymium.home:3000/v1",
        clientSecret: nil,
        password: nil,
        refreshToken: nil
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
    
    /// The full token endpoint URL
    var tokenEndpointURL: URL? {
        URL(string: "\(keycloakURL)/realms/\(realm)/protocol/openid-connect/token")
    }
}

/// Keys for storing secrets (used for credential storage interface)
enum CredentialKey: String {
    case clientSecret
    case password
    case refreshToken
}
