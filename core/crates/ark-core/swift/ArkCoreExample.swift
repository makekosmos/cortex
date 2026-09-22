import Foundation

// Minimal example showing how to use ArkCore from Swift.
// Generated bindings are in generated/ark_coreFFI.h + generated/ark_core.swift
//
// 1. Instantiate ArkCore
// 2. Call startSync with FfiSyncConfig
// 3. Implement ArkEventListenerProtocol to receive events
//
// Full integration happens in apps/delphi/swift/ when macOS is ready.

// MARK: - Example listener implementation

final class ExampleListener: ArkEventListenerProtocol {
    func onEntityChanged(entityJson: String) {
        print("[ArkCore] entity changed: \(entityJson)")
    }

    func onPeerConnected(deviceId: String, deviceName: String) {
        print("[ArkCore] peer connected: \(deviceId) (\(deviceName))")
    }

    func onPeerDisconnected(deviceId: String, remaining: UInt32) {
        print("[ArkCore] peer disconnected: \(deviceId), remaining: \(remaining)")
    }
}

// MARK: - Usage example

func runExample() throws {
    // Instantiate the core object (opens no DB yet).
    let arkCore = ArkCore()

    // Open the database.
    _ = try arkCore.openDb(path: "/tmp/delphi-example.db")

    // Configure sync.
    let config = FfiSyncConfig(
        spaceId: "example-space-id",
        deviceId: "example-device-id",
        deviceName: "Kirill's MacBook",
        port: 21531,
        dbPath: "/tmp/delphi-example.db",
        seedAddresses: [],
        relayUrl: nil,
        relayApiKey: nil
    )

    // Start sync — non-blocking on the calling thread.
    let listener = ExampleListener()
    _ = try arkCore.startSync(config: config, listener: listener)

    print("[ArkCore] sync started")

    // Later: stop sync.
    // _ = try arkCore.stopSync()
}
