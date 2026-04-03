import Foundation
import SwiftData
import os

/// LAN Sync WebSocket client — connects to the Electron hub server.
///
/// Protocol flow:
///   1. Connect to `ws://IP:21531`
///   2. Send `hello` with device_id, device_name, protocol_version=1
///   3. Receive server `hello` back
///   4. Exchange version vectors
///   5. Receive/send sync_changes batches with ACKs
///   6. Enter live mode for realtime mutations
@Observable
@MainActor
final class LanSyncClient {

    // MARK: - Constants

    private static let lanSyncPort = 21531
    private static let protocolVersion = 1
    private static let maxBatchSize = 100
    private static let reconnectDelay: TimeInterval = 5
    private static let maxReconnectDelay: TimeInterval = 60

    // MARK: - State

    enum State: String {
        case disconnected
        case connecting
        case connected
        case syncing
        case live
    }

    var state: State = .disconnected
    var isConnected: Bool { state == .connected || state == .syncing || state == .live }
    var isLive: Bool { state == .live }

    // MARK: - Config

    private var serverIp: String = ""
    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""

    // MARK: - WebSocket

    private var webSocketTask: URLSessionWebSocketTask?
    private var session: URLSession?
    private var receiveTask: Task<Void, Never>?
    private var reconnectTask: Task<Void, Never>?
    private var reconnectAttempt = 0
    private var isRunning = false

    // MARK: - Version vector: entity_id -> HLC string

    private var versionVector: [String: String] = [:]
    private var vectorBuilt = false

    // MARK: - SwiftData

    private var modelContainer: ModelContainer?

    // MARK: - Callbacks

    /// Called when sync data changes and the UI should refresh.
    var onDataChanged: (() -> Void)?

    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "LanSync")

    // MARK: - UserDefaults keys

    private enum Keys {
        static let versionVector = "lan_sync.version_vector"
        static let code = "lan_sync.code"
        static let ip = "lan_sync.ip"
    }

    private let defaults = UserDefaults.standard

    // MARK: - Init

    init() {}

    func setModelContainer(_ container: ModelContainer) {
        self.modelContainer = container
    }

    // MARK: - Public API

    /// Connect to the Electron LAN sync server.
    func connect(ip: String, spaceId: String, deviceId: String, deviceName: String) {
        self.serverIp = ip.trimmingCharacters(in: .whitespaces)
        self.spaceId = spaceId
        self.deviceId = deviceId
        self.deviceName = deviceName
        isRunning = true
        state = .connecting
        reconnectAttempt = 0

        // Persist
        defaults.set(ip, forKey: Keys.ip)

        doConnect()
    }

    /// Connect using a 7-char connection code that encodes the server IP.
    func connectWithCode(_ code: String, spaceId: String, deviceId: String, deviceName: String) -> Bool {
        guard let ip = ConnectionCode.decodeCodeToIp(code) else { return false }

        // Persist the code
        defaults.set(code.replacingOccurrences(of: "-", with: "").uppercased(), forKey: Keys.code)

        connect(ip: ip, spaceId: spaceId, deviceId: deviceId, deviceName: deviceName)
        return true
    }

    func disconnect() {
        isRunning = false
        reconnectTask?.cancel()
        reconnectTask = nil
        receiveTask?.cancel()
        receiveTask = nil
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        session?.invalidateAndCancel()
        session = nil
        vectorBuilt = false
        // Do NOT clear versionVector here — persisted across reconnects
        state = .disconnected
        logger.info("Disconnected")
    }

    /// Clear sync state. Called when leaving a space or clearing data.
    func clearSyncState() {
        versionVector.removeAll()
        vectorBuilt = false
        defaults.removeObject(forKey: Keys.versionVector)
        defaults.removeObject(forKey: Keys.code)
        defaults.removeObject(forKey: Keys.ip)
        logger.info("Sync state cleared")
    }

    /// Get the saved connection code, if any.
    var savedCode: String? {
        defaults.string(forKey: Keys.code)
    }

    /// Get the saved server IP, if any.
    var savedIp: String? {
        defaults.string(forKey: Keys.ip)
    }

    // MARK: - Live change broadcasting

    /// Send a local change to the server in live mode.
    func sendLiveChange(entityType: String, entityId: String, data: [String: Any], deleted: Bool = false) {
        guard let ws = webSocketTask, state == .live else { return }

        let hlc = generateHlc()
        versionVector[entityId] = hlc
        persistVersionVector()

        var entity: [String: Any] = [
            "type": entityType,
            "id": entityId,
            "data": data,
            "hlc": hlc,
        ]
        if deleted { entity["deleted"] = true }

        let msg: [String: Any] = [
            "type": "live_change",
            "change_id": generateId(),
            "entity": entity,
        ]

        sendJSON(msg, on: ws)
    }

    /// Broadcast a TodoItem change via LAN sync.
    func broadcastTodoChange(_ todo: TodoItem) {
        let data = ArkEventMapper.todoToArkEvent(todo)
        // Extract the inner "data" dict for the LAN sync entity format
        let innerData = data["data"] as? [String: Any] ?? data
        sendLiveChange(entityType: "todo", entityId: todo.id.uuidString.lowercased(), data: innerData)
    }

    /// Broadcast a TodoItem deletion via LAN sync.
    func broadcastTodoDelete(_ id: UUID) {
        sendLiveChange(entityType: "todo", entityId: id.uuidString.lowercased(), data: [:], deleted: true)
    }

    /// Broadcast a Project change via LAN sync.
    func broadcastProjectChange(_ project: Project) {
        let data = ArkEventMapper.projectToArkEvent(project)
        let innerData = data["data"] as? [String: Any] ?? data
        sendLiveChange(entityType: "project", entityId: project.id.uuidString.lowercased(), data: innerData)
    }

    /// Broadcast a Project deletion via LAN sync.
    func broadcastProjectDelete(_ id: UUID) {
        sendLiveChange(entityType: "project", entityId: id.uuidString.lowercased(), data: [:], deleted: true)
    }

    /// Broadcast an Area change via LAN sync.
    func broadcastAreaChange(_ area: Area) {
        let data = ArkEventMapper.areaToArkEvent(area)
        let innerData = data["data"] as? [String: Any] ?? data
        sendLiveChange(entityType: "area", entityId: area.id.uuidString.lowercased(), data: innerData)
    }

    /// Broadcast a Tag change via LAN sync.
    func broadcastTagChange(_ tag: Tag) {
        let data = ArkEventMapper.tagToArkEvent(tag)
        let innerData = data["data"] as? [String: Any] ?? data
        sendLiveChange(entityType: "tag", entityId: tag.id.uuidString.lowercased(), data: innerData)
    }

    // MARK: - Internal: Connect

    private func doConnect() {
        guard isRunning else { return }

        let urlString = "ws://\(serverIp):\(Self.lanSyncPort)"
        guard let url = URL(string: urlString) else {
            logger.error("Invalid URL: \(urlString)")
            return
        }

        logger.info("Connecting to \(urlString)")

        let config = URLSessionConfiguration.default
        let newSession = URLSession(configuration: config)
        session = newSession

        let ws = newSession.webSocketTask(with: url)
        webSocketTask = ws
        ws.resume()

        state = .connecting

        // Send hello immediately
        let hello: [String: Any] = [
            "type": "hello",
            "protocol_version": Self.protocolVersion,
            "device_id": deviceId,
            "device_name": deviceName,
            "space_id": spaceId,
        ]
        sendJSON(hello, on: ws)

        // Start receive loop
        startReceiveLoop(ws)
    }

    // MARK: - Internal: Receive loop

    private func startReceiveLoop(_ ws: URLSessionWebSocketTask) {
        receiveTask?.cancel()
        receiveTask = Task { [weak self] in
            while !Task.isCancelled {
                do {
                    let message = try await ws.receive()
                    guard let self else { return }

                    var text: String?
                    switch message {
                    case .string(let s): text = s
                    case .data(let d): text = String(data: d, encoding: .utf8)
                    @unknown default: break
                    }

                    guard let text,
                          let data = text.data(using: .utf8),
                          let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                          let type = json["type"] as? String
                    else { continue }

                    await MainActor.run {
                        self.handleMessage(ws, json: json, type: type)
                    }
                } catch {
                    if !Task.isCancelled {
                        await MainActor.run { [weak self] in
                            guard let self else { return }
                            self.logger.info("Connection lost: \(error.localizedDescription)")
                            self.webSocketTask = nil
                            self.state = .disconnected
                            self.scheduleReconnect()
                        }
                    }
                    break
                }
            }
        }
    }

    // MARK: - Internal: Message handling

    private func handleMessage(_ ws: URLSessionWebSocketTask, json: [String: Any], type: String) {
        switch type {
        case "hello":
            handleHello(ws, json: json)
        case "version_vector":
            handleVersionVector(ws, json: json)
        case "sync_changes":
            handleSyncChanges(ws, json: json)
        case "sync_ack":
            break // ACK for our batches
        case "live_change":
            handleLiveChange(ws, json: json)
        case "live_ack":
            break // ACK received
        case "ping":
            let pong: [String: Any] = ["type": "pong", "ts": json["ts"] ?? 0]
            sendJSON(pong, on: ws)
        case "pong":
            break
        default:
            break
        }
    }

    private func handleHello(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        let version = json["protocol_version"] as? Int ?? 0
        if version != Self.protocolVersion {
            logger.warning("Protocol version mismatch: \(version) vs \(Self.protocolVersion)")
            ws.cancel(with: .protocolError, reason: nil)
            return
        }

        let serverName = json["device_name"] as? String ?? "unknown"
        logger.info("Server hello received: \(serverName)")
        state = .syncing

        // Build and send our version vector
        Task {
            await buildVersionVector()
            vectorBuilt = true

            var vectorDict: [String: Any] = [:]
            for (k, v) in versionVector {
                vectorDict[k] = v
            }
            logger.info("Sending version vector with \(self.versionVector.count) entries")

            let vectorMsg: [String: Any] = [
                "type": "version_vector",
                "vector": vectorDict,
            ]
            sendJSON(vectorMsg, on: ws)
        }
    }

    private func handleVersionVector(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        guard let remoteVector = json["vector"] as? [String: String] else { return }

        Task {
            // Ensure version vector is built
            if !vectorBuilt {
                await buildVersionVector()
                vectorBuilt = true
            }

            // Compute what the server needs from us
            var entitiesToSend: [[String: Any]] = []

            for (entityId, localHlc) in versionVector {
                let remoteHlc = remoteVector[entityId] ?? ""
                if remoteHlc.isEmpty || compareHlc(localHlc, remoteHlc) > 0 {
                    if let entity = await loadEntityById(entityId) {
                        entitiesToSend.append(entity)
                    }
                }
            }

            logger.info("Server needs \(entitiesToSend.count) entities from us")

            // Send in batches
            if entitiesToSend.isEmpty {
                let emptyBatch: [String: Any] = [
                    "type": "sync_changes",
                    "batch_id": generateId(),
                    "entities": [] as [[String: Any]],
                    "is_last": true,
                ]
                sendJSON(emptyBatch, on: ws)
            } else {
                let batches = stride(from: 0, to: entitiesToSend.count, by: Self.maxBatchSize).map {
                    Array(entitiesToSend[$0..<min($0 + Self.maxBatchSize, entitiesToSend.count)])
                }
                for (i, batch) in batches.enumerated() {
                    let batchMsg: [String: Any] = [
                        "type": "sync_changes",
                        "batch_id": generateId(),
                        "entities": batch,
                        "is_last": i == batches.count - 1,
                    ]
                    sendJSON(batchMsg, on: ws)
                }
            }
        }
    }

    private func handleSyncChanges(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        let batchId = json["batch_id"] as? String ?? ""
        let entities = json["entities"] as? [[String: Any]] ?? []
        let isLast = json["is_last"] as? Bool ?? false

        Task {
            var accepted = 0

            for entity in entities {
                if await applySyncEntity(entity) {
                    accepted += 1
                }
            }

            // Persist version vector
            if accepted > 0 {
                persistVersionVector()
            }

            // Send ACK
            let ack: [String: Any] = [
                "type": "sync_ack",
                "batch_id": batchId,
                "accepted": accepted,
            ]
            sendJSON(ack, on: ws)

            if accepted > 0 {
                onDataChanged?()
            }

            if isLast {
                await MainActor.run {
                    self.state = .live
                }
                logger.info("Initial sync complete, entering live mode")
            }
        }
    }

    private func handleLiveChange(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        let changeId = json["change_id"] as? String ?? ""
        guard let entity = json["entity"] as? [String: Any] else { return }

        Task {
            let applied = await applySyncEntity(entity)

            if applied {
                persistVersionVector()
            }

            // Always ACK
            let ack: [String: Any] = [
                "type": "live_ack",
                "change_id": changeId,
            ]
            sendJSON(ack, on: ws)

            if applied {
                onDataChanged?()
            }
        }
    }

    // MARK: - Entity handling

    @MainActor
    private func applySyncEntity(_ entityJson: [String: Any]) async -> Bool {
        guard let container = modelContainer else { return false }

        let entityType = entityJson["type"] as? String ?? ""
        let entityId = entityJson["id"] as? String ?? ""
        let hlc = entityJson["hlc"] as? String ?? ""
        let deleted = entityJson["deleted"] as? Bool ?? false
        let data = entityJson["data"] as? [String: Any]

        guard !entityId.isEmpty else { return false }

        // Check if remote is newer
        let localHlc = versionVector[entityId]
        if let localHlc, !hlc.isEmpty, compareHlc(hlc, localHlc) <= 0 {
            return false // Our version is the same or newer
        }

        let context = container.mainContext

        if deleted {
            guard let uuid = UUID(uuidString: entityId) else { return false }
            switch entityType {
            case "todo":
                let descriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first {
                    context.delete(item)
                }
            case "project":
                let descriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first {
                    context.delete(item)
                }
            case "heading":
                let descriptor = FetchDescriptor<Heading>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first {
                    context.delete(item)
                }
            default:
                break
            }
            try? context.save()
            versionVector[entityId] = hlc.isEmpty ? generateHlc() : hlc
            return true
        }

        guard let data else { return false }

        // Wrap data in the event format that ArkEventMapper expects
        switch entityType {
        case "todo":
            let event: [String: Any] = [
                "source_id": entityId,
                "data": data,
                "event_type": "task",
            ]
            if ArkEventMapper.arkEventToTodo(event, context: context) != nil {
                try? context.save()
                versionVector[entityId] = hlc.isEmpty ? generateHlc() : hlc
                return true
            }
        case "project":
            let event: [String: Any] = [
                "source_id": entityId,
                "data": data,
                "event_type": "project",
            ]
            if ArkEventMapper.arkEventToProject(event, context: context) != nil {
                try? context.save()
                versionVector[entityId] = hlc.isEmpty ? generateHlc() : hlc
                return true
            }
        case "area":
            let event: [String: Any] = [
                "source_id": entityId,
                "data": data,
                "event_type": "area",
            ]
            if ArkEventMapper.arkEventToArea(event, context: context) != nil {
                try? context.save()
                versionVector[entityId] = hlc.isEmpty ? generateHlc() : hlc
                return true
            }
        case "tag":
            let event: [String: Any] = [
                "source_id": entityId,
                "data": data,
                "event_type": "tag",
            ]
            if ArkEventMapper.arkEventToTag(event, context: context) != nil {
                try? context.save()
                versionVector[entityId] = hlc.isEmpty ? generateHlc() : hlc
                return true
            }
        case "heading":
            // Heading not handled by ArkEventMapper; apply directly
            if let title = data["title"] as? String, let uuid = UUID(uuidString: entityId) {
                let descriptor = FetchDescriptor<Heading>(predicate: #Predicate { $0.id == uuid })
                let heading: Heading
                if let existing = try? context.fetch(descriptor).first {
                    heading = existing
                } else {
                    heading = Heading(title: title)
                    heading.id = uuid
                    context.insert(heading)
                }
                heading.title = title
                if let sortOrder = data["sortOrder"] as? Int { heading.sortOrder = sortOrder }
                if let projectIdStr = data["projectId"] as? String, let projectId = UUID(uuidString: projectIdStr) {
                    let projDescriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == projectId })
                    heading.project = try? context.fetch(projDescriptor).first
                }
                try? context.save()
                versionVector[entityId] = hlc.isEmpty ? generateHlc() : hlc
                return true
            }
        default:
            break
        }

        return false
    }

    // MARK: - Version vector

    @MainActor
    private func buildVersionVector() async {
        guard let container = modelContainer else { return }

        // Load persisted vector first
        if versionVector.isEmpty {
            loadPersistedVersionVector()
        }

        let context = container.mainContext
        let todos = (try? context.fetch(FetchDescriptor<TodoItem>())) ?? []
        let projects = (try? context.fetch(FetchDescriptor<Project>())) ?? []
        let areas = (try? context.fetch(FetchDescriptor<Area>())) ?? []
        let tags = (try? context.fetch(FetchDescriptor<Tag>())) ?? []
        let headings = (try? context.fetch(FetchDescriptor<Heading>())) ?? []

        var updated = false

        for todo in todos {
            let id = todo.id.uuidString.lowercased()
            if versionVector[id] == nil {
                versionVector[id] = generateHlc()
                updated = true
            }
        }
        for project in projects {
            let id = project.id.uuidString.lowercased()
            if versionVector[id] == nil {
                versionVector[id] = generateHlc()
                updated = true
            }
        }
        for area in areas {
            let id = area.id.uuidString.lowercased()
            if versionVector[id] == nil {
                versionVector[id] = generateHlc()
                updated = true
            }
        }
        for tag in tags {
            let id = tag.id.uuidString.lowercased()
            if versionVector[id] == nil {
                versionVector[id] = generateHlc()
                updated = true
            }
        }
        for heading in headings {
            let id = heading.id.uuidString.lowercased()
            if versionVector[id] == nil {
                versionVector[id] = generateHlc()
                updated = true
            }
        }

        if updated {
            persistVersionVector()
        }
    }

    @MainActor
    private func loadEntityById(_ entityId: String) async -> [String: Any]? {
        guard let container = modelContainer else { return nil }
        guard let uuid = UUID(uuidString: entityId) else { return nil }

        let context = container.mainContext

        // Try todo
        let todoDescriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == uuid })
        if let todo = try? context.fetch(todoDescriptor).first {
            let event = ArkEventMapper.todoToArkEvent(todo)
            let innerData = event["data"] as? [String: Any] ?? event
            return [
                "type": "todo",
                "id": entityId,
                "data": innerData,
                "hlc": versionVector[entityId] ?? generateHlc(),
            ]
        }

        // Try project
        let projectDescriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == uuid })
        if let project = try? context.fetch(projectDescriptor).first {
            let event = ArkEventMapper.projectToArkEvent(project)
            let innerData = event["data"] as? [String: Any] ?? event
            return [
                "type": "project",
                "id": entityId,
                "data": innerData,
                "hlc": versionVector[entityId] ?? generateHlc(),
            ]
        }

        // Try area
        let areaDescriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == uuid })
        if let area = try? context.fetch(areaDescriptor).first {
            let event = ArkEventMapper.areaToArkEvent(area)
            let innerData = event["data"] as? [String: Any] ?? event
            return [
                "type": "area",
                "id": entityId,
                "data": innerData,
                "hlc": versionVector[entityId] ?? generateHlc(),
            ]
        }

        // Try tag
        let tagDescriptor = FetchDescriptor<Tag>(predicate: #Predicate { $0.id == uuid })
        if let tag = try? context.fetch(tagDescriptor).first {
            let event = ArkEventMapper.tagToArkEvent(tag)
            let innerData = event["data"] as? [String: Any] ?? event
            return [
                "type": "tag",
                "id": entityId,
                "data": innerData,
                "hlc": versionVector[entityId] ?? generateHlc(),
            ]
        }

        // Try heading
        let headingDescriptor = FetchDescriptor<Heading>(predicate: #Predicate { $0.id == uuid })
        if let heading = try? context.fetch(headingDescriptor).first {
            var headingData: [String: Any] = [
                "title": heading.title,
                "sortOrder": heading.sortOrder,
            ]
            if let project = heading.project {
                headingData["projectId"] = project.id.uuidString.lowercased()
            }
            return [
                "type": "heading",
                "id": entityId,
                "data": headingData,
                "hlc": versionVector[entityId] ?? generateHlc(),
            ]
        }

        return nil
    }

    // MARK: - Version vector persistence

    private func loadPersistedVersionVector() {
        guard let raw = defaults.string(forKey: Keys.versionVector),
              let data = raw.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: String]
        else { return }
        versionVector = dict
        logger.info("Loaded persisted version vector with \(dict.count) entries")
    }

    private func persistVersionVector() {
        guard let data = try? JSONSerialization.data(withJSONObject: versionVector),
              let string = String(data: data, encoding: .utf8)
        else { return }
        defaults.set(string, forKey: Keys.versionVector)
    }

    // MARK: - Reconnect

    private func scheduleReconnect() {
        guard isRunning else { return }
        reconnectTask?.cancel()

        let delay = min(Self.reconnectDelay * pow(2.0, Double(reconnectAttempt)), Self.maxReconnectDelay)
        reconnectAttempt += 1

        reconnectTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            guard !Task.isCancelled else { return }
            await MainActor.run { [weak self] in
                guard let self, self.isRunning else { return }
                self.state = .connecting
                self.doConnect()
            }
        }
    }

    // MARK: - HLC utilities

    private func compareHlc(_ a: String, _ b: String) -> Int {
        let partsA = splitHlc(a)
        let partsB = splitHlc(b)

        // Compare wall time
        let timeCmp = partsA.wallTime.compare(partsB.wallTime)
        if timeCmp != .orderedSame { return timeCmp == .orderedAscending ? -1 : 1 }

        // Compare counter
        if partsA.counter != partsB.counter { return partsA.counter < partsB.counter ? -1 : 1 }

        // Compare device ID
        return partsA.deviceId.compare(partsB.deviceId) == .orderedAscending ? -1 :
               partsA.deviceId == partsB.deviceId ? 0 : 1
    }

    private func splitHlc(_ hlc: String) -> (wallTime: String, counter: Int, deviceId: String) {
        // Format: "2026-03-28T14:30:00.123Z:000042:device-id"
        guard let zIndex = hlc.firstIndex(of: "Z") else {
            return (hlc, 0, "")
        }
        let afterZ = hlc.index(after: zIndex)
        guard afterZ < hlc.endIndex, hlc[afterZ] == ":" else {
            return (String(hlc[...zIndex]), 0, "")
        }

        let rest = String(hlc[hlc.index(after: afterZ)...])
        guard let colonIndex = rest.firstIndex(of: ":") else {
            return (String(hlc[...zIndex]), 0, rest)
        }

        let counterStr = String(rest[rest.startIndex..<colonIndex])
        let deviceIdStr = String(rest[rest.index(after: colonIndex)...])
        let counter = Int(counterStr) ?? 0

        return (String(hlc[...zIndex]), counter, deviceIdStr)
    }

    private func generateHlc() -> String {
        let now = ISO8601DateFormatter.lanSync.string(from: Date())
        return "\(now):000000:\(deviceId)"
    }

    private func generateId() -> String {
        "\(Int(Date().timeIntervalSince1970 * 1000))-\(String(Int.random(in: 0..<1_000_000)))"
    }

    // MARK: - Send helper

    private func sendJSON(_ json: [String: Any], on ws: URLSessionWebSocketTask) {
        guard let data = try? JSONSerialization.data(withJSONObject: json),
              let text = String(data: data, encoding: .utf8)
        else { return }

        Task {
            do {
                try await ws.send(.string(text))
            } catch {
                logger.error("Send failed: \(error.localizedDescription)")
            }
        }
    }
}

// MARK: - ISO8601 formatter for LAN sync

private extension ISO8601DateFormatter {
    nonisolated(unsafe) static let lanSync: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
}
