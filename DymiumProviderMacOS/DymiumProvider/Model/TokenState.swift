import Foundation

/// Represents the current authentication state of the token
enum TokenState: Equatable {
    case idle
    case authenticating
    case authenticated(token: String, expiresAt: Date)
    case failed(error: String)
    
    var isAuthenticated: Bool {
        if case .authenticated = self { return true }
        return false
    }
    
    var isAuthenticating: Bool {
        if case .authenticating = self { return true }
        return false
    }
    
    var isFailed: Bool {
        if case .failed = self { return true }
        return false
    }
    
    var errorMessage: String? {
        if case .failed(let error) = self { return error }
        return nil
    }
    
    var token: String? {
        if case .authenticated(let token, _) = self { return token }
        return nil
    }
    
    var expiresAt: Date? {
        if case .authenticated(_, let expiresAt) = self { return expiresAt }
        return nil
    }
    
    static func == (lhs: TokenState, rhs: TokenState) -> Bool {
        switch (lhs, rhs) {
        case (.idle, .idle):
            return true
        case (.authenticating, .authenticating):
            return true
        case (.authenticated(let t1, let e1), .authenticated(let t2, let e2)):
            return t1 == t2 && e1 == e2
        case (.failed(let e1), .failed(let e2)):
            return e1 == e2
        default:
            return false
        }
    }
}

/// Response from Keycloak token endpoint
struct KeycloakTokenResponse: Codable {
    let accessToken: String
    let expiresIn: Int
    let refreshToken: String?
    let refreshExpiresIn: Int?
    let tokenType: String
    
    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case expiresIn = "expires_in"
        case refreshToken = "refresh_token"
        case refreshExpiresIn = "refresh_expires_in"
        case tokenType = "token_type"
    }
}
