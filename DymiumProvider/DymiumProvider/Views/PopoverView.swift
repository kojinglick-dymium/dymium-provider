import SwiftUI

// MARK: - Dymium Brand Colors

extension Color {
    /// Dymium primary blue
    static let dymiumPrimary = Color(red: 0x43/255, green: 0x69/255, blue: 0xff/255)  // #4369ff
    /// Dymium accent purple
    static let dymiumAccent = Color(red: 0x90/255, green: 0x4d/255, blue: 0xff/255)   // #904dff
    /// Dymium success green
    static let dymiumSuccess = Color(red: 0x19/255, green: 0x87/255, blue: 0x54/255)  // #198754
    /// Dymium danger red
    static let dymiumDanger = Color(red: 0xdc/255, green: 0x35/255, blue: 0x45/255)   // #dc3545
    /// Dymium warning yellow
    static let dymiumWarning = Color(red: 0xff/255, green: 0xc1/255, blue: 0x07/255)  // #ffc107
}

/// Main popover view shown when clicking the menu bar icon
struct PopoverView: View {
    @ObservedObject var tokenService: TokenService
    @Environment(\.openWindow) private var openWindow
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header with ghost and status
            HStack {
                ghostIcon
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Dymium Provider")
                        .font(.headline)
                    Text(statusText)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Spacer()
            }
            
            Divider()
            
            // Status details
            if tokenService.hasCredentials {
                statusDetails
            } else {
                setupPrompt
            }
            
            Divider()
            
            // Actions
            HStack {
                Button("Refresh Now") {
                    Task {
                        await tokenService.manualRefresh()
                    }
                }
                .disabled(tokenService.state.isAuthenticating || !tokenService.hasCredentials)
                
                Spacer()
                
                if tokenService.hasCredentials {
                    Button("Log Out") {
                        Task {
                            await tokenService.logOut()
                        }
                    }
                    .foregroundColor(.dymiumDanger)
                }
                
                Button("Setup") {
                    openWindow(id: "setup")
                }
                
                Button("Quit") {
                    NSApplication.shared.terminate(nil)
                }
            }
            .buttonStyle(.bordered)
        }
        .padding()
        .frame(width: 300)
    }
    
    @ViewBuilder
    private var ghostIcon: some View {
        switch tokenService.state {
        case .idle, .failed:
            GhostShape()
                .stroke(Color.gray, lineWidth: 1.5)
                .frame(width: 18, height: 24)
        case .authenticating:
            AnimatedGradientGhost()
                .frame(width: 18, height: 24)
        case .authenticated:
            GradientGhost()
                .frame(width: 18, height: 24)
        }
    }
    
    private var statusColor: Color {
        switch tokenService.state {
        case .idle:
            return .gray
        case .authenticating:
            return .dymiumPrimary  // Will be overridden by gradient in ghostIcon
        case .authenticated:
            return .dymiumPrimary  // Will be overridden by gradient in ghostIcon
        case .failed:
            return .gray
        }
    }
    
    private var statusText: String {
        switch tokenService.state {
        case .idle:
            return "Not started"
        case .authenticating:
            return "Authenticating..."
        case .authenticated:
            return "Authenticated"
        case .failed(let error):
            return "Failed: \(error)"
        }
    }
    
    @ViewBuilder
    private var statusDetails: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let lastRefresh = tokenService.lastRefreshTime {
                HStack {
                    Text("Last refresh:")
                        .foregroundColor(.secondary)
                    Spacer()
                    Text(lastRefresh, style: .relative)
                        .monospacedDigit()
                }
            }
            
            if let expiresAt = tokenService.state.expiresAt {
                HStack {
                    Text("Expires:")
                        .foregroundColor(.secondary)
                    Spacer()
                    Text(expiresAt, style: .relative)
                        .monospacedDigit()
                }
            }
            
            if let error = tokenService.state.errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.dymiumDanger)
                    .lineLimit(3)
            }
            
            // Token file status
            HStack {
                Text("Token file:")
                    .foregroundColor(.secondary)
                Spacer()
                if TokenWriter.shared.tokenExists {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.dymiumSuccess)
                    Text("~/.dymium/token")
                        .font(.caption)
                        .foregroundColor(.secondary)
                } else {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.dymiumDanger)
                    Text("Not written")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
        .font(.system(.body, design: .monospaced))
    }
    
    @ViewBuilder
    private var setupPrompt: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Credentials not configured")
                .font(.subheadline)
                .foregroundColor(.dymiumWarning)
            
            Text("Click Setup to enter your Keycloak credentials.")
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }
}

/// Setup window for entering credentials and configuration
struct SetupView: View {
    @ObservedObject var tokenService: TokenService
    @Binding var isPresented: Bool
    @Environment(\.dismiss) private var dismiss
    
    // Connection settings
    @State private var keycloakURL: String = ""
    @State private var realm: String = ""
    @State private var clientId: String = ""
    @State private var username: String = ""
    @State private var llmEndpoint: String = ""
    @State private var ghostllmApp: String = ""
    
    // Secrets (stored in Keychain)
    @State private var clientSecret: String = ""
    @State private var password: String = ""
    
    @State private var errorMessage: String?
    @State private var isSaving = false
    @State private var showAdvanced = false
    
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            HStack {
                GhostShape()
                    .stroke(Color.dymiumPrimary, lineWidth: 2)
                    .frame(width: 24, height: 32)
                
                VStack(alignment: .leading) {
                    Text("Dymium Setup")
                        .font(.title2)
                        .fontWeight(.semibold)
                    Text("Configure your Keycloak connection")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            
            Divider()
            
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    // Connection Settings
                    GroupBox("Connection") {
                        VStack(alignment: .leading, spacing: 12) {
                            fieldRow(label: "Keycloak URL", placeholder: "https://192.168.50.100:9173", text: $keycloakURL)
                            fieldRow(label: "Username", placeholder: "user@example.com", text: $username)
                            fieldRow(label: "LLM Endpoint", placeholder: "http://spoofcorp.llm.dymium.home:3000/v1", text: $llmEndpoint)
                            fieldRow(label: "GhostLLM App", placeholder: "your-ghostllm-app-name", text: $ghostllmApp)
                        }
                        .padding(.vertical, 4)
                    }
                    
                    // Credentials (secrets)
                    GroupBox("Credentials") {
                        VStack(alignment: .leading, spacing: 12) {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Client Secret")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                SecureField("Client secret from Keycloak", text: $clientSecret)
                                    .textFieldStyle(.roundedBorder)
                            }
                            
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Password")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                SecureField("Your password", text: $password)
                                    .textFieldStyle(.roundedBorder)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    
                    // Advanced settings (collapsible)
                    DisclosureGroup("Advanced Settings", isExpanded: $showAdvanced) {
                        VStack(alignment: .leading, spacing: 12) {
                            fieldRow(label: "Realm", placeholder: "dymium", text: $realm)
                            fieldRow(label: "Client ID", placeholder: "dymium", text: $clientId)
                        }
                        .padding(.top, 8)
                    }
                    
                    // Info text
                    Text("Secrets are stored securely in the macOS Keychain. Configuration is saved to ~/.dymium/config.json")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                        .padding(.top, 8)
                }
            }
            .frame(maxHeight: 320)
            
            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.dymiumDanger)
                    .lineLimit(3)
            }
            
            Divider()
            
            HStack {
                Button("Cancel") {
                    closeWindow()
                }
                .keyboardShortcut(.cancelAction)
                
                Spacer()
                
                Button("Save & Connect") {
                    saveAndConnect()
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(!isFormValid || isSaving)
            }
        }
        .padding(20)
        .frame(width: 420, height: 520)
        .onAppear {
            loadCurrentConfig()
        }
    }
    
    @ViewBuilder
    private func fieldRow(label: String, placeholder: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)
            TextField(placeholder, text: text)
                .textFieldStyle(.roundedBorder)
        }
    }
    
    private var isFormValid: Bool {
        !keycloakURL.isEmpty &&
        !username.isEmpty &&
        !clientSecret.isEmpty &&
        !password.isEmpty &&
        !realm.isEmpty &&
        !clientId.isEmpty &&
        !llmEndpoint.isEmpty &&
        !ghostllmApp.isEmpty
    }
    
    private func loadCurrentConfig() {
        let config = tokenService.config
        keycloakURL = config.keycloakURL
        realm = config.realm
        clientId = config.clientId
        username = config.username
        llmEndpoint = config.llmEndpoint
        ghostllmApp = config.ghostllmApp ?? ""
        // Secrets are not pre-filled for security
    }
    
    private func closeWindow() {
        dismiss()
        // Also close the NSWindow if dismiss doesn't work
        NSApplication.shared.keyWindow?.close()
    }
    
    private func saveAndConnect() {
        isSaving = true
        errorMessage = nil
        
        do {
            try tokenService.saveSetup(
                keycloakURL: keycloakURL,
                realm: realm,
                clientId: clientId,
                username: username,
                llmEndpoint: llmEndpoint,
                ghostllmApp: ghostllmApp,
                clientSecret: clientSecret,
                password: password
            )
            
            // Close the window
            closeWindow()
            
            // Trigger authentication
            Task {
                await tokenService.manualRefresh()
            }
        } catch {
            errorMessage = error.localizedDescription
        }
        
        isSaving = false
    }
}
