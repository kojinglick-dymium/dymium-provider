import Foundation

/// Service responsible for syncing available models from the LLM endpoint to OpenCode config
final class ModelSyncService {
    static let shared = ModelSyncService()
    
    private init() {}
    
    /// Response from /v1/models endpoint
    struct ModelsResponse: Codable {
        let data: [Model]
        let object: String
        
        struct Model: Codable {
            let id: String
            let object: String
            let created: Int
            let ownedBy: String
            
            enum CodingKeys: String, CodingKey {
                case id, object, created
                case ownedBy = "owned_by"
            }
        }
    }
    
    /// Model capabilities configuration for OpenCode
    /// These are reasonable defaults for different model types
    private func modelConfig(for modelId: String) -> [String: Any] {
        // Determine capabilities based on model name patterns
        let isClaudeOpus = modelId.lowercased().contains("opus")
        let isReasoning = isClaudeOpus || modelId.lowercased().contains("o1") || modelId.lowercased().contains("o3")
        let isLarge = isClaudeOpus || modelId.lowercased().contains("pro") || modelId.lowercased().contains("5.2")
        
        // Context and output limits based on model type
        let contextLimit: Int
        let outputLimit: Int
        
        if modelId.lowercased().contains("claude") {
            contextLimit = 200000
            outputLimit = 16384
        } else if modelId.lowercased().contains("gpt") || modelId.lowercased().contains("o1") || modelId.lowercased().contains("o3") {
            contextLimit = 128000
            outputLimit = 16384
        } else if modelId.lowercased().contains("gemini") {
            contextLimit = isLarge ? 1000000 : 128000
            outputLimit = 8192
        } else {
            // Default for unknown models
            contextLimit = 32000
            outputLimit = 4096
        }
        
        // Generate a friendly display name
        let displayName = generateDisplayName(for: modelId)
        
        return [
            "name": displayName,
            "tool_call": true,
            "temperature": true,
            "attachment": true,
            "reasoning": isReasoning,
            "limit": [
                "context": contextLimit,
                "output": outputLimit
            ]
        ]
    }
    
    /// Generate a friendly display name from model ID
    private func generateDisplayName(for modelId: String) -> String {
        // Map common model IDs to friendly names
        let mappings: [String: String] = [
            "claude-opus-4-5": "Claude Opus 4.5",
            "claude-sonnet-4-5": "Claude Sonnet 4.5",
            "claude-sonnet-4": "Claude Sonnet 4",
            "gpt-5.2-chat-latest": "GPT-5.2",
            "gpt-5-mini": "GPT-5 Mini",
            "gemini-2.5-pro": "Gemini 2.5 Pro",
            "gemini-2.5-flash": "Gemini 2.5 Flash",
            "gemini-3-flash-preview": "Gemini 3 Flash Preview",
            "gemini-3-pro-preview": "Gemini 3 Pro Preview",
            "llama": "Llama",
        ]
        
        if let mapped = mappings[modelId] {
            return "\(mapped) (GhostLLM)"
        }
        
        // Fallback: capitalize and clean up the model ID
        let cleaned = modelId
            .replacingOccurrences(of: "-", with: " ")
            .replacingOccurrences(of: "_", with: " ")
            .split(separator: " ")
            .map { $0.prefix(1).uppercased() + $0.dropFirst() }
            .joined(separator: " ")
        
        return "\(cleaned) (GhostLLM)"
    }
    
    /// Fetch available models from the LLM endpoint
    func fetchAvailableModels(token: String) async throws -> [String] {
        let config = AppConfig.load()
        guard let url = URL(string: "\(config.llmEndpoint)/models") else {
            throw ModelSyncError.invalidURL
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        // Set Host header to hostname only (Istio best practice)
        if let host = url.host {
            request.setValue(host, forHTTPHeaderField: "Host")
        }
        request.timeoutInterval = 30
        
        print("[ModelSync] Fetching models from \(url.absoluteString)")
        
        let (data, response) = try await URLSession.shared.data(for: request)
        
        guard let httpResponse = response as? HTTPURLResponse else {
            throw ModelSyncError.invalidResponse
        }
        
        guard httpResponse.statusCode == 200 else {
            let body = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw ModelSyncError.fetchFailed(statusCode: httpResponse.statusCode, body: body)
        }
        
        let modelsResponse = try JSONDecoder().decode(ModelsResponse.self, from: data)
        let modelIds = modelsResponse.data.map { $0.id }
        
        print("[ModelSync] Found \(modelIds.count) models: \(modelIds.joined(separator: ", "))")
        
        return modelIds
    }
    
    /// Sync models from LLM endpoint to OpenCode config
    /// Returns true if config was updated
    @discardableResult
    func syncModels(token: String) async throws -> Bool {
        let availableModels = try await fetchAvailableModels(token: token)
        return try updateOpenCodeConfig(with: availableModels)
    }
    
    /// Update OpenCode config with the available models
    private func updateOpenCodeConfig(with modelIds: [String]) throws -> Bool {
        let configPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/opencode/opencode.json")
        
        // Read existing config
        guard FileManager.default.fileExists(atPath: configPath.path),
              let data = try? Data(contentsOf: configPath),
              var config = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            print("[ModelSync] No existing opencode.json found, skipping model sync")
            return false
        }
        
        // Get the provider section
        guard var providers = config["provider"] as? [String: Any],
              var dymiumProvider = providers["dymium"] as? [String: Any] else {
            print("[ModelSync] No dymium provider found in config, skipping model sync")
            return false
        }
        
        // Get current models
        let currentModels = dymiumProvider["models"] as? [String: Any] ?? [:]
        let currentModelIds = Set(currentModels.keys)
        let newModelIds = Set(modelIds)
        
        // Check if there are any changes
        if currentModelIds == newModelIds {
            print("[ModelSync] Models are already in sync (\(currentModelIds.count) models)")
            return false
        }
        
        // Build new models dictionary
        var newModels: [String: Any] = [:]
        for modelId in modelIds {
            if let existingConfig = currentModels[modelId] {
                // Keep existing config for known models
                newModels[modelId] = existingConfig
            } else {
                // Add new model with default config
                newModels[modelId] = modelConfig(for: modelId)
                print("[ModelSync] Adding new model: \(modelId)")
            }
        }
        
        // Log removed models
        let removedModels = currentModelIds.subtracting(newModelIds)
        for modelId in removedModels {
            print("[ModelSync] Removing model no longer available: \(modelId)")
        }
        
        // Update the config
        dymiumProvider["models"] = newModels
        providers["dymium"] = dymiumProvider
        config["provider"] = providers
        
        // Write back
        let jsonData = try JSONSerialization.data(
            withJSONObject: config,
            options: [.prettyPrinted, .sortedKeys]
        )
        try jsonData.write(to: configPath, options: .atomic)
        
        print("[ModelSync] ✅ Updated opencode.json with \(modelIds.count) models")
        return true
    }
}

// MARK: - Errors

enum ModelSyncError: LocalizedError {
    case invalidURL
    case invalidResponse
    case fetchFailed(statusCode: Int, body: String)
    
    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "Invalid LLM endpoint URL"
        case .invalidResponse:
            return "Invalid response from LLM endpoint"
        case .fetchFailed(let statusCode, let body):
            return "Failed to fetch models (\(statusCode)): \(body)"
        }
    }
}
