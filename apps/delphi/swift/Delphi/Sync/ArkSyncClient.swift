import Foundation
import SwiftData
import os

/// WebSocket client that syncs SwiftData models with the Ark server.
///
/// Protocol flow (see SYNC.md):
/// 1. Connect to ws://host/ws/sync?key=<api_key>
/// 2. Send sync_start with device_id and version vector
/// 3. Receive sync_changes from server (changes we haven't seen)
/// 4. Send sync_changes to server (our local outbox)
/// 5. Realtime: send/receive individual changes
@Observable
@MainActor
final class ArkSyncClient {

    // MARK: - Published state

    var isConnected = false
    var isSyncing = false
    var lastSyncAt: Date?
    var pendingChanges: Int = 0

    // MARK: - Dependencies

    private let settings: SyncSettings
    private var modelContainer: ModelContainer?

    // MARK: - WebSocket

    private var webSocketTask: URLSessionWebSocketTask?
    private var session: URLSession?
    private var heartbeatTask: Task<Void, Never>?
    private var receiveTask: Task<Void, Never>?
    private var reconnectTask: Task<Void, Never>?
    private var reconnectAttempt = 0
    private let maxReconnectDelay: TimeInterval = 60

    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "ArkSync")

    // MARK: - Local outbox (in-memory, persisted as version vector in settings)

    /// Changes made locally while connected or offline, pending send.
    private var outbox: [[String: Any]] = []

    // MARK: - Init

    init(settings: SyncSettings) {
        self.settings = settings
    }

    func setModelContainer(_ container: ModelContainer) {
        self.modelContainer = container
    }

    // MARK: - Connect / Disconnect

    func connect() {
        guard settings.isConfigured else {
            logger.warning("Sync not configured, skipping connect")
            return
        }
        guard !isConnected else { return }

        reconnectAttempt = 0
        performConnect()
    }

    func disconnect() {
        reconnectTask?.cancel()
        reconnectTask = nil
        heartbeatTask?.cancel()
        heartbeatTask = nil
        receiveTask?.cancel()
        receiveTask = nil
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        isConnected = false
    }

    // MARK: - Send local changes

    /// Enqueue a local change to be sent to the server.
    func sendChange(_ event: [String: Any], changeType: String) {
        var change = event
        change["change_type"] = changeType

        if isConnected, let ws = webSocketTask {
            let message: [String: Any] = [
                "type": "change",
                "event_id": event["source_id"] as? String ?? UUID().uuidString,
                "change_type": changeType,
                "data": event,
            ]

            Task {
                do {
                    let data = try JSONSerialization.data(withJSONObject: message)
                    try await ws.send(.string(String(data: data, encoding: .utf8)!))
                } catch {
                    logger.error("Failed to send change: \(error.localizedDescription)")
                    outbox.append(change)
                    pendingChanges = outbox.count
                }
            }
        } else {
            outbox.append(change)
            pendingChanges = outbox.count
        }
    }

    /// Convenience: send a TodoItem change.
    func sendTodoChange(_ todo: TodoItem, changeType: String = "update") {
        let event = ArkEventMapper.todoToArkEvent(todo, changeType: changeType)
        sendChange(event, changeType: changeType)
    }

    /// Convenience: send a Project change.
    func sendProjectChange(_ project: Project, changeType: String = "update") {
        let event = ArkEventMapper.projectToArkEvent(project, changeType: changeType)
        sendChange(event, changeType: changeType)
    }

    /// Manual sync trigger: re-sends outbox.
    func syncNow() {
        guard isConnected else {
            connect()
            return
        }
        Task { await flushOutbox() }
    }

    // MARK: - Internal: Connection

    private func performConnect() {
        let urlString = settings.serverUrl.trimmingCharacters(in: .whitespacesAndNewlines)

        // Build WebSocket URL
        var base = urlString
        if base.hasPrefix("http://") {
            base = "ws://" + base.dropFirst(7)
        } else if base.hasPrefix("https://") {
            base = "wss://" + base.dropFirst(8)
        } else if !base.hasPrefix("ws://") && !base.hasPrefix("wss://") {
            base = "ws://" + base
        }
        if !base.contains("/ws/sync") {
            base = base.hasSuffix("/") ? base + "ws/sync" : base + "/ws/sync"
        }

        guard var components = URLComponents(string: base) else {
            logger.error("Invalid server URL: \(urlString)")
            return
        }

        var queryItems = components.queryItems ?? []
        queryItems.append(URLQueryItem(name: "key", value: settings.apiKey))
        components.queryItems = queryItems

        guard let url = components.url else {
            logger.error("Failed to build WebSocket URL")
            return
        }

        let config = URLSessionConfiguration.default
        config.waitsForConnectivity = true
        session = URLSession(configuration: config)

        let ws = session!.webSocketTask(with: url)
        webSocketTask = ws
        ws.resume()

        // Send sync_start
        Task {
            do {
                let syncStart: [String: Any] = [
                    "type": "sync_start",
                    "device_id": settings.deviceId,
                    "device_name": Host.current().localizedName ?? "Mac",
                    "platform": "macos",
                    "vector": settings.versionVector,
                ]
                let data = try JSONSerialization.data(withJSONObject: syncStart)
                try await ws.send(.string(String(data: data, encoding: .utf8)!))
                logger.info("Sent sync_start to \(url.absoluteString)")

                isConnected = true
                reconnectAttempt = 0

                startReceiving()
                startHeartbeat()
            } catch {
                logger.error("Failed to send sync_start: \(error.localizedDescription)")
                handleDisconnect()
            }
        }
    }

    // MARK: - Receive loop

    private func startReceiving() {
        receiveTask?.cancel()
        receiveTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                do {
                    guard let ws = self.webSocketTask else { break }
                    let message = try await ws.receive()
                    switch message {
                    case .string(let text):
                        await self.handleMessage(text)
                    case .data(let data):
                        if let text = String(data: data, encoding: .utf8) {
                            await self.handleMessage(text)
                        }
                    @unknown default:
                        break
                    }
                } catch {
                    if !Task.isCancelled {
                        self.logger.error("WebSocket receive error: \(error.localizedDescription)")
                        await MainActor.run { self.handleDisconnect() }
                    }
                    break
                }
            }
        }
    }

    private func handleMessage(_ text: String) async {
        guard let data = text.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = json["type"] as? String
        else { return }

        switch type {
        case "sync_changes":
            await handleSyncChanges(json)

        case "change":
            await handleIncomingChange(json)

        case "sync_ack":
            let applied = json["applied"] as? Int ?? 0
            logger.info("Server acknowledged \(applied) changes")

        case "change_ack":
            if let eventId = json["event_id"] as? String,
               let seq = json["device_seq"] as? Int {
                settings.updateVector(deviceId: settings.deviceId, seq: seq)
                logger.info("Change acknowledged: \(eventId) seq=\(seq)")
            }

        case "ping":
            // Respond with pong
            if let ws = webSocketTask {
                let pong = try? JSONSerialization.data(withJSONObject: ["type": "pong"])
                if let pong, let str = String(data: pong, encoding: .utf8) {
                    try? await ws.send(.string(str))
                }
            }

        case "error":
            let msg = json["message"] as? String ?? "unknown"
            logger.error("Server error: \(msg)")

        default:
            logger.warning("Unknown message type: \(type)")
        }
    }

    // MARK: - Handle incoming sync_changes

    private func handleSyncChanges(_ json: [String: Any]) async {
        guard let changes = json["changes"] as? [[String: Any]] else { return }
        guard !changes.isEmpty else {
            logger.info("No new changes from server")
            await flushOutbox()
            lastSyncAt = Date()
            return
        }

        isSyncing = true
        logger.info("Received \(changes.count) changes from server")

        guard let container = modelContainer else {
            logger.error("No ModelContainer set")
            isSyncing = false
            return
        }

        let context = ModelContext(container)
        context.autosaveEnabled = false

        for change in changes {
            applyRemoteChange(change, context: context)

            // Update version vector
            if let deviceId = change["device_id"] as? String,
               let seq = change["device_seq"] as? Int {
                settings.updateVector(deviceId: deviceId, seq: seq)
            }
        }

        do {
            try context.save()
            logger.info("Applied \(changes.count) remote changes")
        } catch {
            logger.error("Failed to save remote changes: \(error.localizedDescription)")
        }

        isSyncing = false
        lastSyncAt = Date()

        // Now send our pending outbox
        await flushOutbox()
    }

    // MARK: - Handle single incoming change (realtime)

    private func handleIncomingChange(_ json: [String: Any]) async {
        guard let container = modelContainer else { return }

        let context = ModelContext(container)
        context.autosaveEnabled = false

        applyRemoteChange(json, context: context)

        if let deviceId = json["device_id"] as? String,
           let seq = json["device_seq"] as? Int {
            settings.updateVector(deviceId: deviceId, seq: seq)
        }

        do {
            try context.save()
        } catch {
            logger.error("Failed to save incoming change: \(error.localizedDescription)")
        }
    }

    // MARK: - Apply a remote change to SwiftData

    private func applyRemoteChange(_ change: [String: Any], context: ModelContext) {
        let changeType = change["change_type"] as? String ?? "create"
        let eventData = change["data"] as? [String: Any] ?? change
        let eventType = eventData["event_type"] as? String
            ?? (change["data"] as? [String: Any])?["event_type"] as? String
            ?? "task"

        if changeType == "delete" {
            handleRemoteDeletion(change, eventType: eventType, context: context)
            return
        }

        switch eventType {
        case "task":
            _ = ArkEventMapper.arkEventToTodo(eventData, context: context)
        case "project":
            _ = ArkEventMapper.arkEventToProject(eventData, context: context)
        case "area":
            _ = ArkEventMapper.arkEventToArea(eventData, context: context)
        case "tag":
            _ = ArkEventMapper.arkEventToTag(eventData, context: context)
        default:
            logger.warning("Unknown event_type: \(eventType)")
        }
    }

    private func handleRemoteDeletion(_ change: [String: Any], eventType: String, context: ModelContext) {
        guard let sourceId = change["event_id"] as? String
                ?? (change["data"] as? [String: Any])?["source_id"] as? String,
              let uuid = UUID(uuidString: sourceId)
        else { return }

        switch eventType {
        case "task":
            let descriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        case "project":
            let descriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        case "area":
            let descriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        case "tag":
            let descriptor = FetchDescriptor<Tag>(predicate: #Predicate { $0.id == uuid })
            if let item = try? context.fetch(descriptor).first {
                context.delete(item)
            }
        default:
            break
        }
    }

    // MARK: - Flush outbox

    private func flushOutbox() async {
        guard !outbox.isEmpty, let ws = webSocketTask, isConnected else { return }

        let changes = outbox
        outbox = []
        pendingChanges = 0

        let message: [String: Any] = [
            "type": "sync_changes",
            "changes": changes.map { change -> [String: Any] in
                [
                    "event_id": change["source_id"] as? String ?? UUID().uuidString,
                    "change_type": change["change_type"] as? String ?? "create",
                    "data": change,
                ]
            },
        ]

        do {
            let data = try JSONSerialization.data(withJSONObject: message)
            try await ws.send(.string(String(data: data, encoding: .utf8)!))
            logger.info("Flushed \(changes.count) outbox changes")
        } catch {
            // Put them back
            outbox.insert(contentsOf: changes, at: 0)
            pendingChanges = outbox.count
            logger.error("Failed to flush outbox: \(error.localizedDescription)")
        }
    }

    // MARK: - Heartbeat

    private func startHeartbeat() {
        heartbeatTask?.cancel()
        heartbeatTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(30))
                guard !Task.isCancelled, let self, let ws = self.webSocketTask else { break }
                let pong = try? JSONSerialization.data(withJSONObject: ["type": "pong"])
                if let pong, let str = String(data: pong, encoding: .utf8) {
                    try? await ws.send(.string(str))
                }
            }
        }
    }

    // MARK: - Reconnect with exponential backoff

    private func handleDisconnect() {
        isConnected = false
        webSocketTask = nil
        heartbeatTask?.cancel()
        receiveTask?.cancel()

        guard settings.isAutoSyncEnabled else { return }

        reconnectTask?.cancel()
        reconnectTask = Task { [weak self] in
            guard let self else { return }
            let delay = min(pow(2.0, Double(self.reconnectAttempt)), self.maxReconnectDelay)
            self.logger.info("Reconnecting in \(delay)s (attempt \(self.reconnectAttempt + 1))")

            try? await Task.sleep(for: .seconds(delay))

            guard !Task.isCancelled else { return }
            self.reconnectAttempt += 1
            self.performConnect()
        }
    }
}
