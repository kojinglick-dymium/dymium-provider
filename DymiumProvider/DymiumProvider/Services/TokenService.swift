import Foundation
import Combine

/// Service responsible for authenticating with Keycloak and managing tokens
@MainActor
final class TokenService: ObservableObject {
    static let shared = TokenService()
    
    @Published private(set) var state: TokenState = .idle
    @Published private(set) var lastRefreshTime: Date?
    @Published private(set) var config: AppConfig
    
    private let keychain = KeychainService.shared
    private var refreshTimer: Timer?
    private var urlSession: URLSession
    
    private init() {
        self.config = AppConfig.load()
        
        // Create a URLSession that trusts self-signed certificates
        // This is needed for the local Keycloak instance
        let sessionConfig = URLSessionConfiguration.default
        self.urlSession = URLSession(
            configuration: sessionConfig,
            delegate: InsecureURLSessionDelegate(),
            delegateQueue: nil
        )
    }
    
    /// Reload configuration from disk
    func reloadConfig() {
        self.config = AppConfig.load()
    }
    
    /// Start the token refresh loop
    func startRefreshLoop() {
        // Immediately attempt authentication
        Task {
            await authenticate()
        }
        
        // Set up the refresh timer
        refreshTimer?.invalidate()
        refreshTimer = Timer.scheduledTimer(
            withTimeInterval: TimeInterval(config.refreshIntervalSeconds),
            repeats: true
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                await self?.refreshTokenIfNeeded()
            }
        }
    }
    
    /// Stop the refresh loop
    func stopRefreshLoop() {
        refreshTimer?.invalidate()
        refreshTimer = nil
    }
    
    /// Manually trigger a refresh
    func manualRefresh() async {
        await authenticate()
    }
    
    /// Authenticate - tries refresh token first, falls back to password grant
    private func authenticate() async {
        state = .authenticating
        
        // Try refresh token first if we have one
        if let refreshToken = try? keychain.load(key: CredentialKey.refreshToken) {
            print("[TokenService] Attempting refresh token grant...")
            do {
                let response = try await performRefreshTokenGrant(refreshToken: refreshToken)
                print("[TokenService] Refresh token grant succeeded, new token expires in \(response.expiresIn)s")
                await handleSuccessfulAuth(response: response)
                return
            } catch {
                print("[TokenService] Refresh token grant failed: \(error)")
                print("[TokenService] Falling back to password grant...")
                // Fall through to password grant
            }
        } else {
            print("[TokenService] No refresh token found, using password grant")
        }
        
        // Fall back to password grant
        do {
            let response = try await performPasswordGrant()
            print("[TokenService] Password grant succeeded, token expires in \(response.expiresIn)s")
            await handleSuccessfulAuth(response: response)
        } catch {
            print("[TokenService] Password grant failed: \(error)")
            state = .failed(error: error.localizedDescription)
        }
    }
    
    /// Refresh token only if needed (called by timer)
    private func refreshTokenIfNeeded() async {
        // Skip if we're already authenticating
        if case .authenticating = state { return }
        
        await authenticate()
    }
    
    /// Handle a successful authentication response
    private func handleSuccessfulAuth(response: KeycloakTokenResponse) async {
        let expiresAt = Date().addingTimeInterval(TimeInterval(response.expiresIn))
        
        // Store the refresh token if we got one
        if let refreshToken = response.refreshToken {
            do {
                try keychain.save(refreshToken, forKey: CredentialKey.refreshToken)
                if let refreshExpiresIn = response.refreshExpiresIn {
                    print("[TokenService] New refresh token saved, expires in \(refreshExpiresIn)s")
                } else {
                    print("[TokenService] New refresh token saved")
                }
            } catch {
                print("[TokenService] Failed to save refresh token: \(error)")
            }
        } else {
            print("[TokenService] Warning: No refresh token in response - will need password grant next time")
        }
        
        // Write the access token to disk
        do {
            try TokenWriter.shared.writeToken(response.accessToken)
            print("[TokenService] Access token written to \(AppConfig.tokenPath.path)")
        } catch {
            print("[TokenService] Failed to write token to disk: \(error)")
        }
        
        // Ensure opencode config has the dymium provider (only adds if missing)
        do {
            try OpenCodeConfigService.shared.ensureDymiumProvider()
        } catch {
            print("[TokenService] Failed to ensure opencode config: \(error)")
        }
        
        // Always update the auth.json with the fresh token
        OpenCodeConfigService.shared.updateToken()
        print("[TokenService] Updated auth.json with fresh token, expires at \(expiresAt)")
        
        // Sync available models from the LLM endpoint
        Task {
            do {
                let updated = try await ModelSyncService.shared.syncModels(token: response.accessToken)
                if updated {
                    print("[TokenService] Model sync completed - config was updated")
                } else {
                    print("[TokenService] Model sync completed - no changes needed")
                }
            } catch {
                print("[TokenService] Model sync failed: \(error)")
                // Don't fail the auth flow if model sync fails
            }
        }
        
        state = .authenticated(token: response.accessToken, expiresAt: expiresAt)
        lastRefreshTime = Date()
    }
    
    /// Perform password grant authentication
    private func performPasswordGrant() async throws -> KeycloakTokenResponse {
        guard let url = config.tokenEndpointURL else {
            throw TokenError.invalidURL
        }
        
        guard let clientSecret = try keychain.load(key: CredentialKey.clientSecret) else {
            throw TokenError.missingClientSecret
        }
        
        guard let password = try keychain.load(key: CredentialKey.password) else {
            throw TokenError.missingPassword
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        
        let body = [
            "grant_type": "password",
            "client_id": config.clientId,
            "client_secret": clientSecret,
            "username": config.username,
            "password": password
        ]
        
        request.httpBody = body
            .map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? $0.value)" }
            .joined(separator: "&")
            .data(using: .utf8)
        
        let (data, response) = try await urlSession.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw TokenError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            let errorBody = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw TokenError.authFailed(statusCode: httpResponse.statusCode, body: errorBody)
        }
        
        return try JSONDecoder().decode(KeycloakTokenResponse.self, from: data)
    }
    
    /// Perform refresh token grant
    private func performRefreshTokenGrant(refreshToken: String) async throws -> KeycloakTokenResponse {
        guard let url = config.tokenEndpointURL else {
            throw TokenError.invalidURL
        }
        
        guard let clientSecret = try keychain.load(key: CredentialKey.clientSecret) else {
            throw TokenError.missingClientSecret
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        
        let body = [
            "grant_type": "refresh_token",
            "client_id": config.clientId,
            "client_secret": clientSecret,
            "refresh_token": refreshToken
        ]
        
        request.httpBody = body
            .map { "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? $0.value)" }
            .joined(separator: "&")
            .data(using: .utf8)
        
        let (data, response) = try await urlSession.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw TokenError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            let errorBody = String(data: data, encoding: .utf8) ?? "Unknown error"
            print("[TokenService] Refresh token grant failed with status \(httpResponse.statusCode): \(errorBody)")
            
            // Only clear the refresh token if it's definitively invalid (400/401)
            // For other errors (network issues, server errors), keep it for retry
            if httpResponse.statusCode == 400 || httpResponse.statusCode == 401 {
                print("[TokenService] Clearing invalid refresh token")
                try? keychain.delete(key: CredentialKey.refreshToken)
            } else {
                print("[TokenService] Keeping refresh token for retry (server error or network issue)")
            }
            
            throw TokenError.authFailed(statusCode: httpResponse.statusCode, body: errorBody)
        }
        
        return try JSONDecoder().decode(KeycloakTokenResponse.self, from: data)
    }
    
    /// Save configuration and credentials (called from setup UI)
    func saveSetup(
        keycloakURL: String,
        realm: String,
        clientId: String,
        username: String,
        llmEndpoint: String,
        clientSecret: String,
        password: String
    ) throws {
        // Save config to disk
        let newConfig = AppConfig(
            keycloakURL: keycloakURL,
            clientId: clientId,
            username: username,
            realm: realm,
            refreshIntervalSeconds: config.refreshIntervalSeconds,
            llmEndpoint: llmEndpoint
        )
        try newConfig.save()
        
        // Update in-memory config
        self.config = newConfig
        
        // Save secrets to config file
        try keychain.save(clientSecret, forKey: CredentialKey.clientSecret)
        try keychain.save(password, forKey: CredentialKey.password)
        
        // Clear any old refresh token since credentials changed
        try? keychain.delete(key: CredentialKey.refreshToken)
    }
    
    /// Check if credentials are configured
    var hasCredentials: Bool {
        keychain.exists(key: CredentialKey.clientSecret) && keychain.exists(key: CredentialKey.password)
    }
}

// MARK: - Errors

enum TokenError: LocalizedError {
    case invalidURL
    case missingClientSecret
    case missingPassword
    case invalidResponse
    case authFailed(statusCode: Int, body: String)
    
    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "Invalid Keycloak URL"
        case .missingClientSecret:
            return "Client secret not configured"
        case .missingPassword:
            return "Password not configured"
        case .invalidResponse:
            return "Invalid response from server"
        case .authFailed(let statusCode, let body):
            return "Auth failed (\(statusCode)): \(body)"
        }
    }
}

// MARK: - URLSession Delegate for self-signed certs

private class InsecureURLSessionDelegate: NSObject, URLSessionDelegate {
    func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        // Trust all certificates (like curl -k)
        // This is needed for the local Keycloak with self-signed cert
        if challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
           let serverTrust = challenge.protectionSpace.serverTrust {
            let credential = URLCredential(trust: serverTrust)
            completionHandler(.useCredential, credential)
        } else {
            completionHandler(.performDefaultHandling, nil)
        }
    }
}
