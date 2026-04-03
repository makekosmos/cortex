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
        session?.invalidateAndCancel()
        session = nil
        isConnected = false
    }

    // MARK: - Send local changes

    /// Enqueue a local change to be sent to the server.
    func sendChange(_ event: [String: Any], changeType: String) {
        var change = event
        change["change_type"] = changeType

        let sourceId = event["source_id"] as? String ?? "?"
        logger.info("sendChange called: \(changeType) \(sourceId) connected=\(self.isConnected) ws=\(self.webSocketTask != nil)")

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
                    guard let text = String(data: data, encoding: .utf8) else {
                        logger.error("sendChange: failed to encode as UTF-8")
                        return
                    }
                    try await ws.send(.string(text))
                    logger.info("sendChange: sent \(changeType) \(sourceId) (\(text.count) bytes)")
                } catch {
                    logger.error("sendChange: ws.send failed: \(error.localizedDescription)")
                    outbox.append(change)
                    pendingChanges = outbox.count
                }
            }
        } else {
            logger.warning("sendChange: offline, queuing \(changeType) \(sourceId) (outbox=\(self.outbox.count + 1))")
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

    /// Clear all local data and resync from server.
    func clearLocalData() {
        disconnect()

        guard let container = modelContainer else { return }
        let context = container.mainContext

        // Delete all SwiftData records one-by-one to avoid batch delete
        // constraint violations from inverse relationships.
        do {
            // Todos first (they reference Project/Area/Tag)
            let todos = try context.fetch(FetchDescriptor<TodoItem>())
            for item in todos { context.delete(item) }

            let projects = try context.fetch(FetchDescriptor<Project>())
            for item in projects { context.delete(item) }

            let areas = try context.fetch(FetchDescriptor<Area>())
            for item in areas { context.delete(item) }

            let tags = try context.fetch(FetchDescriptor<Tag>())
            for item in tags { context.delete(item) }

            let headings = try context.fetch(FetchDescriptor<Heading>())
            for item in headings { context.delete(item) }

            try context.save()
            logger.info("Cleared all local data")
        } catch {
            logger.error("Failed to clear local data: \(error.localizedDescription)")
        }

        // Reset sync state
        settings.resetVector()
        settings.serverEpoch = nil
        outbox = []
        pendingChanges = 0

        // Reconnect — empty vector triggers full sync
        connect()
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

        session?.invalidateAndCancel()
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
                guard let text = String(data: data, encoding: .utf8) else {
                    logger.error("Failed to encode sync_start as UTF-8")
                    return
                }
                try await ws.send(.string(text))
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
        guard let ws = webSocketTask else { return }
        receiveTask = Task { [weak self, ws] in
            while !Task.isCancelled {
                do {
                    let message = try await ws.receive()
                    guard let self else { return }
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
                        await MainActor.run { [weak self] in
                            self?.logger.error("WebSocket receive error: \(error.localizedDescription)")
                            self?.handleDisconnect()
                        }
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

        // --- Epoch / full-sync detection ---
        let isFullSync = (json["is_full_sync"] as? Bool) ?? false
        let incomingEpoch = json["server_epoch"] as? String
        let storedEpoch = settings.serverEpoch
        let epochChanged = incomingEpoch != nil && storedEpoch != nil && incomingEpoch != storedEpoch

        if epochChanged {
            logger.warning("Server epoch changed — resetting version vector, will push full local state")
            settings.resetVector()
        }
        if let epoch = incomingEpoch {
            settings.serverEpoch = epoch
        }

        isSyncing = true

        // Collect task source_ids the server sent us (for zombie cleanup & sendMissing)
        var serverTaskIds = Set<String>()

        if !changes.isEmpty {
            logger.info("Received \(changes.count) changes from server (fullSync=\(isFullSync), epochChanged=\(epochChanged))")

            guard let container = modelContainer else {
                logger.error("No ModelContainer set")
                isSyncing = false
                return
            }

            let context = container.mainContext

            for change in changes {
                applyRemoteChange(change, context: context)

                // Collect task ids for reconciliation
                if let data = change["data"] as? [String: Any] {
                    let eventType = data["event_type"] as? String ?? ""
                    if eventType == "task" {
                        if let sourceId = data["source_id"] as? String {
                            serverTaskIds.insert(sourceId.lowercased())
                        }
                    }
                }
                if let eventId = change["event_id"] as? String {
                    serverTaskIds.insert(eventId.lowercased())
                }

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
        } else {
            logger.info("No new changes from server (fullSync=\(isFullSync), epochChanged=\(epochChanged))")
        }

        // Zombie cleanup: remove local tasks missing from the server snapshot.
        // ONLY on a genuine full sync without an epoch change — if epoch changed the server
        // was wiped and we must NOT delete user data based on an empty/partial snapshot.
        if isFullSync && !epochChanged && !serverTaskIds.isEmpty {
            await removeZombies(serverTaskIds: serverTaskIds)
        }

        // Push local tasks missing on the server.
        // Only run on full sync or when the epoch changed — NOT on every incremental reconnect.
        if isFullSync || epochChanged {
            await pushLocalTodos(serverTaskIds: serverTaskIds)
        }

        isSyncing = false
        lastSyncAt = Date()

        await flushOutbox()
    }

    // MARK: - Push local todos missing on server (full sync / epoch change only)

    /// Send local TodoItems that the server snapshot doesn't contain.
    /// `serverTaskIds` is the set of lowercase source_ids from the full-sync batch.
    private func pushLocalTodos(serverTaskIds: Set<String>) async {
        guard let container = modelContainer else { return }

        let context = container.mainContext
        let descriptor = FetchDescriptor<TodoItem>()
        guard let todos = try? context.fetch(descriptor) else { return }

        var pushed = 0
        for todo in todos {
            let idStr = todo.id.uuidString.lowercased()
            if serverTaskIds.contains(idStr) { continue }

            let event = ArkEventMapper.todoToArkEvent(todo, changeType: "create")
            sendChange(event, changeType: "create")
            pushed += 1
        }

        if pushed > 0 {
            logger.info("Pushed \(pushed) local todos missing on server")
        }
    }

    // MARK: - Zombie cleanup (full sync only, epoch unchanged)

    /// Delete local tasks that the server doesn't know about AND aren't in the outbox.
    /// Guards: only called when isFullSync=true AND epochChanged=false.
    private func removeZombies(serverTaskIds: Set<String>) async {
        guard let container = modelContainer else { return }

        let context = container.mainContext
        let descriptor = FetchDescriptor<TodoItem>()
        guard let todos = try? context.fetch(descriptor) else { return }

        // Build outbox id set so we never delete tasks pending upload
        let outboxIds = Set(outbox.compactMap { $0["source_id"] as? String }.map { $0.lowercased() })

        var removed = 0
        for todo in todos {
            let idStr = todo.id.uuidString.lowercased()
            if !serverTaskIds.contains(idStr) && !outboxIds.contains(idStr) {
                context.delete(todo)
                removed += 1
            }
        }

        if removed > 0 {
            logger.info("Removed \(removed) zombie tasks not present on server")
            try? context.save()
        }
    }

    // MARK: - Handle single incoming change (realtime)

    private func handleIncomingChange(_ json: [String: Any]) async {
        guard let container = modelContainer else { return }

        // Use mainContext so SwiftUI @Query views see changes immediately
        let context = container.mainContext

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

        logger.info("Applying remote change: type=\(eventType) changeType=\(changeType) eventId=\(change["event_id"] as? String ?? "?")")

        if changeType == "delete" {
            handleRemoteDeletion(change, eventType: eventType, context: context)
            return
        }

        switch eventType {
        case "task":
            let result = ArkEventMapper.arkEventToTodo(eventData, context: context)
            logger.info("arkEventToTodo result: \(result?.title ?? "nil")")
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

        let pending = outbox
        outbox = []
        pendingChanges = 0

        // Re-read current SwiftData state for non-delete events so we never
        // send stale payloads that were enqueued before a remote update arrived.
        let context = modelContainer?.mainContext
        let freshChanges: [[String: Any]] = pending.map { change in
            let changeType = change["change_type"] as? String ?? "create"
            let eventType = change["event_type"] as? String ?? "task"

            // Deletes carry no payload to freshen; non-task events are fine as-is.
            guard changeType != "delete", eventType == "task" else { return change }

            guard let context,
                  let sourceIdStr = change["source_id"] as? String,
                  let uuid = UUID(uuidString: sourceIdStr)
            else { return change }

            let descriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == uuid })
            if let current = try? context.fetch(descriptor).first {
                // Rebuild payload from the live SwiftData object.
                var fresh = ArkEventMapper.todoToArkEvent(current, changeType: changeType)
                // Preserve any extra keys the original change carried (e.g. hlc timestamp).
                for (key, value) in change where fresh[key] == nil {
                    fresh[key] = value
                }
                return fresh
            }
            // Task was deleted locally after being enqueued — skip stale entry.
            logger.info("Outbox: task \(sourceIdStr) no longer exists, skipping stale entry")
            return change
        }

        let message: [String: Any] = [
            "type": "sync_changes",
            "changes": freshChanges.map { change -> [String: Any] in
                [
                    "event_id": change["source_id"] as? String ?? UUID().uuidString,
                    "change_type": change["change_type"] as? String ?? "create",
                    "data": change,
                ]
            },
        ]

        do {
            let data = try JSONSerialization.data(withJSONObject: message)
            guard let text = String(data: data, encoding: .utf8) else {
                outbox.insert(contentsOf: pending, at: 0)
                pendingChanges = outbox.count
                return
            }
            try await ws.send(.string(text))
            logger.info("Flushed \(freshChanges.count) outbox changes")
        } catch {
            // Put the original pending items back (not the freshened copies).
            outbox.insert(contentsOf: pending, at: 0)
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
