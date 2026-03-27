import Foundation

@Observable
final class SyncSettings {
    private enum Keys {
        static let serverUrl = "ark.sync.serverUrl"
        static let apiKey = "ark.sync.apiKey"
        static let deviceId = "ark.sync.deviceId"
        static let isAutoSyncEnabled = "ark.sync.autoSync"
        static let versionVector = "ark.sync.versionVector"
    }

    private let defaults = UserDefaults.standard

    var serverUrl: String {
        didSet { defaults.set(serverUrl, forKey: Keys.serverUrl) }
    }

    var apiKey: String {
        didSet { defaults.set(apiKey, forKey: Keys.apiKey) }
    }

    let deviceId: String

    var isAutoSyncEnabled: Bool {
        didSet { defaults.set(isAutoSyncEnabled, forKey: Keys.isAutoSyncEnabled) }
    }

    /// Version vector: tracks the last seen device_seq per device_id.
    var versionVector: [String: Int] {
        didSet { defaults.set(versionVector, forKey: Keys.versionVector) }
    }

    var isConfigured: Bool {
        !serverUrl.isEmpty && !apiKey.isEmpty
    }

    var isPaired: Bool {
        isConfigured
    }

    init() {
        serverUrl = defaults.string(forKey: Keys.serverUrl) ?? ""
        apiKey = defaults.string(forKey: Keys.apiKey) ?? ""
        isAutoSyncEnabled = defaults.bool(forKey: Keys.isAutoSyncEnabled)
        versionVector = (defaults.dictionary(forKey: Keys.versionVector) as? [String: Int]) ?? [:]

        // Generate a stable device ID on first launch
        if let existing = defaults.string(forKey: Keys.deviceId) {
            deviceId = existing
        } else {
            let newId = "mac-\(UUID().uuidString.prefix(8).lowercased())"
            defaults.set(newId, forKey: Keys.deviceId)
            deviceId = newId
        }
    }

    func updateVector(deviceId: String, seq: Int) {
        let current = versionVector[deviceId] ?? 0
        if seq > current {
            versionVector[deviceId] = seq
        }
    }
}
