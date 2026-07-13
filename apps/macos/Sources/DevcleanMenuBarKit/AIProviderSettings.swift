import Foundation
import Security

public enum AIProviderKind: String, CaseIterable, Identifiable, Sendable {
    case appleOnDevice = "apple-on-device"
    case deepSeek = "deepseek"

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .appleOnDevice:
            "Apple On-Device"
        case .deepSeek:
            "DeepSeek"
        }
    }

    public var processingLabel: String {
        switch self {
        case .appleOnDevice:
            "On device"
        case .deepSeek:
            "Remote API"
        }
    }

    public static func selected(from defaults: UserDefaults = .standard) -> AIProviderKind {
        guard let rawValue = defaults.string(forKey: PreferenceKeys.aiInsightsProvider) else {
            return .appleOnDevice
        }
        return AIProviderKind(rawValue: rawValue) ?? .appleOnDevice
    }
}

public protocol AISecretStoring: Sendable {
    func read(account: String) throws -> String?
    func write(_ secret: String, account: String) throws
    func delete(account: String) throws
}

public enum AIKeychainAccount {
    public static let deepSeek = "deepseek-api-key"
}

public enum AIKeychainError: LocalizedError {
    case unexpectedStatus(OSStatus)
    case invalidSecret

    public var errorDescription: String? {
        switch self {
        case .unexpectedStatus:
            "DevCleaner could not access the selected AI credential in Keychain."
        case .invalidSecret:
            "Enter a valid API key before saving."
        }
    }
}

public struct AIKeychainStore: AISecretStoring, @unchecked Sendable {
    public static let service = "com.tuanle.devclean.ai"

    public init() {}

    public func read(account: String) throws -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Self.service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw AIKeychainError.unexpectedStatus(status)
        }
        guard let data = result as? Data,
            let secret = String(data: data, encoding: .utf8)
        else {
            throw AIKeychainError.invalidSecret
        }
        return secret
    }

    public func write(_ secret: String, account: String) throws {
        let normalized = secret.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty, let data = normalized.data(using: .utf8) else {
            throw AIKeychainError.invalidSecret
        }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Self.service,
            kSecAttrAccount as String: account,
        ]
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecSuccess {
            return
        }
        guard updateStatus == errSecItemNotFound else {
            throw AIKeychainError.unexpectedStatus(updateStatus)
        }
        var insert = query
        for (key, value) in attributes {
            insert[key] = value
        }
        let insertStatus = SecItemAdd(insert as CFDictionary, nil)
        guard insertStatus == errSecSuccess else {
            throw AIKeychainError.unexpectedStatus(insertStatus)
        }
    }

    public func delete(account: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Self.service,
            kSecAttrAccount as String: account,
        ]
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw AIKeychainError.unexpectedStatus(status)
        }
    }
}

@MainActor
public final class AIProviderCredentialsController: ObservableObject {
    @Published public var draftDeepSeekKey = ""
    @Published public private(set) var hasDeepSeekKey = false
    @Published public private(set) var statusMessage: String?
    @Published public private(set) var errorMessage: String?

    private let store: any AISecretStoring

    public init(store: any AISecretStoring = AIKeychainStore()) {
        self.store = store
        refresh()
    }

    public func refresh() {
        do {
            hasDeepSeekKey = try store.read(account: AIKeychainAccount.deepSeek) != nil
            errorMessage = nil
        } catch {
            hasDeepSeekKey = false
            errorMessage = error.localizedDescription
        }
    }

    public func saveDeepSeekKey() {
        do {
            try store.write(draftDeepSeekKey, account: AIKeychainAccount.deepSeek)
            draftDeepSeekKey = ""
            hasDeepSeekKey = true
            statusMessage = "DeepSeek API key saved in macOS Keychain."
            errorMessage = nil
        } catch {
            statusMessage = nil
            errorMessage = error.localizedDescription
        }
    }

    public func removeDeepSeekKey() {
        do {
            try store.delete(account: AIKeychainAccount.deepSeek)
            draftDeepSeekKey = ""
            hasDeepSeekKey = false
            statusMessage = "DeepSeek API key removed from macOS Keychain."
            errorMessage = nil
        } catch {
            statusMessage = nil
            errorMessage = error.localizedDescription
        }
    }
}
