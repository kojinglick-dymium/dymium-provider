import Foundation

/// Service responsible for writing the token to disk
/// The token is written to ~/.dymium/token for opencode to read
final class TokenWriter {
    static let shared = TokenWriter()
    
    private let tokenPath = AppConfig.tokenPath
    private let configDirectory = AppConfig.configDirectory
    
    private init() {}
    
    /// Write the access token to ~/.dymium/token
    func writeToken(_ token: String) throws {
        // Ensure the directory exists
        try FileManager.default.createDirectory(
            at: configDirectory,
            withIntermediateDirectories: true,
            attributes: nil
        )
        
        // Write the token with restrictive permissions (owner read/write only)
        let data = Data(token.utf8)
        
        // First write to the file
        try data.write(to: tokenPath, options: .atomic)
        
        // Then set permissions to 0600 (owner read/write only)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: tokenPath.path
        )
        
        print("Token written to \(tokenPath.path)")
    }
    
    /// Read the current token from disk (if any)
    func readToken() -> String? {
        guard let data = try? Data(contentsOf: tokenPath),
              let token = String(data: data, encoding: .utf8) else {
            return nil
        }
        return token.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    
    /// Delete the token file
    func deleteToken() throws {
        if FileManager.default.fileExists(atPath: tokenPath.path) {
            try FileManager.default.removeItem(at: tokenPath)
        }
    }
    
    /// Check if a token file exists
    var tokenExists: Bool {
        FileManager.default.fileExists(atPath: tokenPath.path)
    }
}
