import Foundation

/// Service for storing and retrieving credentials from the config file
/// (Simplified alternative to macOS Keychain to avoid keychain access prompts)
final class KeychainService {
    static let shared = KeychainService()
    
    private init() {}
    
    /// Save a string value to the config file
    func save(_ value: String, forKey key: CredentialKey) throws {
        var config = AppConfig.load()
        
        switch key {
        case .clientSecret:
            config.clientSecret = value
        case .password:
            config.password = value
        case .refreshToken:
            config.refreshToken = value
        }
        
        try config.save()
    }
    
    /// Load a string value from the config file
    func load(key: CredentialKey) throws -> String? {
        let config = AppConfig.load()
        
        switch key {
        case .clientSecret:
            return config.clientSecret
        case .password:
            return config.password
        case .refreshToken:
            return config.refreshToken
        }
    }
    
    /// Delete a value from the config file
    func delete(key: CredentialKey) throws {
        var config = AppConfig.load()
        
        switch key {
        case .clientSecret:
            config.clientSecret = nil
        case .password:
            config.password = nil
        case .refreshToken:
            config.refreshToken = nil
        }
        
        try config.save()
    }
    
    /// Check if a key exists in the config
    func exists(key: CredentialKey) -> Bool {
        do {
            return try load(key: key) != nil
        } catch {
            return false
        }
    }
}

enum CredentialError: LocalizedError {
    case saveFailed(error: Error)
    case loadFailed(error: Error)
    case deleteFailed(error: Error)
    case invalidData
    
    var errorDescription: String? {
        switch self {
        case .saveFailed(let error):
            return "Failed to save credential: \(error.localizedDescription)"
        case .loadFailed(let error):
            return "Failed to load credential: \(error.localizedDescription)"
        case .deleteFailed(let error):
            return "Failed to delete credential: \(error.localizedDescription)"
        case .invalidData:
            return "Invalid credential data"
        }
    }
}
