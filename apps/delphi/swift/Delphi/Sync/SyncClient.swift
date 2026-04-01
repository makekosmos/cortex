import Foundation
import os

/// WebSocket client for LAN sync -- connects outbound to a peer's server.
///
/// Equal-peer model: every device runs both a WS server and WS clients.
/// This is the client half -- one instance per remote peer.
///
/// Features:
/// - Takes a PeerRecord with multiple addresses
/// - Races connections to ALL addresses in parallel
/// - First successful connection = active, others cancelled
/// - Full protocol: hello -> version_vector -> batch -> peer_list -> live
/// - Auto-reconnect with exponential backoff
@Observable
@MainActor
final class SyncClient {

    // MARK: - Constants

    private static let connectTimeoutSec: TimeInterval = 5
    private static let reconnectBaseSec: TimeInterval = 2
    private static let reconnectMaxSec: TimeInterval = 30

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

    // MARK: - Config

    private(set) var peer: PeerRecord
    private let spaceId: String
    private let deviceId: String
    private let deviceName: String
    private var ownAddresses: [String]

    // MARK: - Callbacks

    var onEntityReceived: ((SyncEntity) -> Void)?
    var onPeerConnected: ((String, String) -> Void)?   // (deviceId, deviceName)
    var onPeerDisconnected: ((String) -> Void)?
    var onPeerListReceived: (([PeerRecord]) -> Void)?

    /// Version vector delegates (shared with PeerManager)
    var loadVersionVector: (() -> VersionVector)?
    var saveVersionVector: ((VersionVector) -> Void)?
    var loadAllEntities: (() -> [SyncEntity])?

    // MARK: - Internal

    private var webSocketTask: URLSessionWebSocketTask?
    private var session: URLSession?
    private var receiveTask: Task<Void, Never>?
    private var reconnectTask: Task<Void, Never>?
    private var reconnectDelay: TimeInterval
    private var stopped = true
    private var authenticated = false
    private var syncComplete = false
    private var queuedLiveChanges: [SyncEntity] = []
    private var peerDeviceName: String = ""

    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "SyncClient")

    // MARK: - Init

    init(peer: PeerRecord, spaceId: String, deviceId: String, deviceName: String, ownAddresses: [String]) {
        self.peer = peer
        self.spaceId = spaceId
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.ownAddresses = ownAddresses
        self.reconnectDelay = Self.reconnectBaseSec
    }

    // MARK: - Public API

    func start() {
        stopped = false
        connect()
    }

    func stop() {
        stopped = true
        reconnectTask?.cancel()
        reconnectTask = nil
        receiveTask?.cancel()
        receiveTask = nil
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        session?.invalidateAndCancel()
        session = nil
        authenticated = false
        syncComplete = false
        queuedLiveChanges.removeAll()
        state = .disconnected
    }

    func updatePeer(_ newPeer: PeerRecord) {
        self.peer = newPeer
    }

    func updateOwnAddresses(_ addresses: [String]) {
        self.ownAddresses = addresses
    }

    /// Send a live change through this client connection.
    func sendLiveChange(_ entity: SyncEntity) {
        guard let ws = webSocketTask, authenticated else { return }

        if !syncComplete {
            queuedLiveChanges.append(entity)
            return
        }

        let changeId = generateSyncId()
        let msg: [String: Any] = [
            "type": "live_change",
            "change_id": changeId,
            "entity": entityToDict(entity),
        ]
        sendJSON(msg, on: ws)
    }

    // MARK: - Connection: Race All Addresses

    private func connect() {
        guard !stopped else { return }

        let addresses = peer.addresses
        guard !addresses.isEmpty else {
            logger.warning("No addresses for peer \(self.peer.device_name), scheduling reconnect")
            scheduleReconnect()
            return
        }

        state = .connecting

        // Sort: last_address first
        let lastAddr = peer.last_address
        let sorted = addresses.sorted { a, b in
            if a == lastAddr { return true }
            if b == lastAddr { return false }
            return false
        }

        logger.info("Connecting to \(self.peer.device_name): racing \(sorted.count) addresses")

        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = Self.connectTimeoutSec
        let newSession = URLSession(configuration: config)
        session = newSession

        var connected = false
        var pendingCount = sorted.count
        var candidates: [URLSessionWebSocketTask] = []

        for addr in sorted {
            let url: URL?
            if addr.contains("://") {
                url = URL(string: "ws\(addr.dropFirst(3))") // tcp:// -> ws://
            } else {
                url = URL(string: "ws://\(addr)")
            }
            guard let wsUrl = url else {
                pendingCount -= 1
                continue
            }

            let candidate = newSession.webSocketTask(with: wsUrl)
            candidates.append(candidate)
            candidate.resume()

            // Monitor open via trying to receive (and also send hello)
            let addr = addr
            Task { [weak self] in
                guard let self else { return }

                // Try sending hello immediately -- if connection fails, this will throw
                do {
                    // Wait a tiny bit to allow TCP to establish
                    try await Task.sleep(for: .milliseconds(200))

                    guard !connected, !self.stopped else {
                        if !connected { candidate.cancel(with: .goingAway, reason: nil) }
                        return
                    }

                    // Send hello
                    let hello = self.buildHelloMessage()
                    guard let helloData = try? JSONSerialization.data(withJSONObject: hello),
                          let helloText = String(data: helloData, encoding: .utf8)
                    else { return }

                    try await candidate.send(.string(helloText))

                    // Wait for hello response
                    let response = try await candidate.receive()
                    var text: String?
                    switch response {
                    case .string(let s): text = s
                    case .data(let d): text = String(data: d, encoding: .utf8)
                    @unknown default: break
                    }

                    guard let text,
                          let data = text.data(using: .utf8),
                          let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                          let type = json["type"] as? String,
                          type == "hello"
                    else {
                        candidate.cancel(with: .normalClosure, reason: nil)
                        await MainActor.run {
                            pendingCount -= 1
                            if !connected && pendingCount <= 0 {
                                self.scheduleReconnect()
                            }
                        }
                        return
                    }

                    await MainActor.run {
                        guard !connected, !self.stopped else {
                            candidate.cancel(with: .goingAway, reason: nil)
                            return
                        }
                        connected = true

                        // Cancel other candidates
                        for other in candidates where other !== candidate {
                            other.cancel(with: .goingAway, reason: nil)
                        }

                        // Update peer record
                        let cleanAddr = addr.hasPrefix("tcp://") ? String(addr.dropFirst(6)) : addr
                        self.peer.last_address = cleanAddr

                        self.webSocketTask = candidate
                        self.reconnectDelay = Self.reconnectBaseSec
                        self.logger.info("Connected to \(self.peer.device_name) via \(addr)")

                        self.handleHelloResponse(json)
                        self.startReceiveLoop(candidate)
                    }

                } catch {
                    await MainActor.run {
                        pendingCount -= 1
                        if !connected && pendingCount <= 0 && !self.stopped {
                            self.logger.warning("All addresses failed for \(self.peer.device_name)")
                            self.scheduleReconnect()
                        }
                    }
                }
            }
        }

        // Overall timeout
        Task {
            try? await Task.sleep(for: .seconds(15))
            await MainActor.run {
                if !connected && !self.stopped && self.state == .connecting {
                    for c in candidates {
                        c.cancel(with: .goingAway, reason: nil)
                    }
                    self.scheduleReconnect()
                }
            }
        }
    }

    private func buildHelloMessage() -> [String: Any] {
        [
            "type": "hello",
            "protocol_version": SYNC_PROTOCOL_VERSION,
            "device_id": deviceId,
            "device_name": deviceName,
            "space_id": spaceId,
            "addresses": ownAddresses,
        ]
    }

    // MARK: - Hello Response

    private func handleHelloResponse(_ json: [String: Any]) {
        authenticated = true
        peerDeviceName = json["device_name"] as? String ?? peer.device_name
        state = .syncing

        onPeerConnected?(peer.device_id, peerDeviceName)

        // Send our version vector
        sendVersionVector()
    }

    // MARK: - Receive Loop

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
                            let wasAuth = self.authenticated
                            self.authenticated = false
                            self.syncComplete = false
                            self.webSocketTask = nil
                            self.state = .disconnected

                            if wasAuth {
                                self.logger.info("Disconnected from \(self.peerDeviceName)")
                                self.onPeerDisconnected?(self.peer.device_id)
                            }
                            if !self.stopped {
                                self.scheduleReconnect()
                            }
                        }
                    }
                    break
                }
            }
        }
    }

    // MARK: - Message Handling

    private func handleMessage(_ ws: URLSessionWebSocketTask, json: [String: Any], type: String) {
        switch type {
        case "version_vector":
            handleVersionVector(ws, json: json)
        case "sync_changes":
            handleSyncChanges(ws, json: json)
        case "sync_ack":
            break
        case "live_change":
            handleLiveChange(ws, json: json)
        case "live_ack":
            break
        case "peer_list":
            handlePeerList(json)
        case "ping":
            sendJSON(["type": "pong", "ts": json["ts"] ?? 0], on: ws)
        case "pong":
            break
        default:
            break
        }
    }

    // MARK: - Version Vector

    private func sendVersionVector() {
        guard let ws = webSocketTask else { return }

        var vector = loadVersionVector?() ?? [:]

        if vector.isEmpty {
            if let entities = loadAllEntities?() {
                for entity in entities {
                    if vector[entity.id] == nil {
                        vector[entity.id] = entity.hlc
                    }
                }
                saveVersionVector?(vector)
            }
        }

        let msg: [String: Any] = [
            "type": "version_vector",
            "vector": vector,
        ]
        sendJSON(msg, on: ws)
    }

    private func handleVersionVector(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        guard let remoteVector = json["vector"] as? [String: String] else { return }

        var localVector = loadVersionVector?() ?? [:]
        let allEntities = loadAllEntities?() ?? []

        // Update vector
        var vectorUpdated = false
        for entity in allEntities {
            if localVector[entity.id] == nil {
                localVector[entity.id] = entity.hlc
                vectorUpdated = true
            }
        }
        if vectorUpdated {
            saveVersionVector?(localVector)
        }

        // Compute what remote needs
        var entitiesToSend: [SyncEntity] = []
        for entity in allEntities {
            let remoteHlc = remoteVector[entity.id]
            if remoteHlc == nil || isNewerHlc(entity.hlc, remoteHlc!) {
                entitiesToSend.append(entity)
            }
        }

        if !entitiesToSend.isEmpty {
            logger.info("Sending \(entitiesToSend.count) entities to \(self.peerDeviceName)")
            sendBatches(entitiesToSend, ws: ws)
        } else {
            let msg: [String: Any] = [
                "type": "sync_changes",
                "batch_id": generateSyncId(),
                "entities": [] as [[String: Any]],
                "is_last": true,
            ]
            sendJSON(msg, on: ws)
        }

        syncComplete = true
        state = .live
        flushQueuedLiveChanges()
    }

    // MARK: - Sync Batches

    private func sendBatches(_ entities: [SyncEntity], ws: URLSessionWebSocketTask) {
        let batches = stride(from: 0, to: entities.count, by: MAX_BATCH_SIZE).map {
            Array(entities[$0..<min($0 + MAX_BATCH_SIZE, entities.count)])
        }

        for (i, batch) in batches.enumerated() {
            let msg: [String: Any] = [
                "type": "sync_changes",
                "batch_id": generateSyncId(),
                "entities": batch.map { entityToDict($0) },
                "is_last": i == batches.count - 1,
            ]
            sendJSON(msg, on: ws)
        }
    }

    private func handleSyncChanges(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        let batchId = json["batch_id"] as? String ?? ""
        let entities = json["entities"] as? [[String: Any]] ?? []
        let isLast = json["is_last"] as? Bool ?? false

        var localVector = loadVersionVector?() ?? [:]
        var accepted = 0

        for entityDict in entities {
            guard let entityType = entityDict["type"] as? String,
                  let entityId = entityDict["id"] as? String,
                  let hlc = entityDict["hlc"] as? String
            else { continue }

            let localHlc = localVector[entityId]
            if localHlc == nil || isNewerHlc(hlc, localHlc!) {
                let deleted = entityDict["deleted"] as? Bool ?? false
                let data = entityDict["data"] as? [String: Any] ?? [:]
                let codableData = data.mapValues { AnyCodable($0) }

                let entity = SyncEntity(type: entityType, id: entityId, data: codableData, hlc: hlc, deleted: deleted)
                onEntityReceived?(entity)
                localVector[entityId] = hlc
                accepted += 1
            }
        }

        saveVersionVector?(localVector)

        let ack: [String: Any] = [
            "type": "sync_ack",
            "batch_id": batchId,
            "accepted": accepted,
        ]
        sendJSON(ack, on: ws)

        if isLast {
            syncComplete = true
            state = .live
            flushQueuedLiveChanges()
            logger.info("Received all sync batches from \(self.peerDeviceName)")
        }
    }

    // MARK: - Live Mode

    private func handleLiveChange(_ ws: URLSessionWebSocketTask, json: [String: Any]) {
        let changeId = json["change_id"] as? String ?? ""
        guard let entityDict = json["entity"] as? [String: Any],
              let entityType = entityDict["type"] as? String,
              let entityId = entityDict["id"] as? String,
              let hlc = entityDict["hlc"] as? String
        else { return }

        var localVector = loadVersionVector?() ?? [:]
        let localHlc = localVector[entityId]

        if localHlc == nil || isNewerHlc(hlc, localHlc!) {
            let deleted = entityDict["deleted"] as? Bool ?? false
            let data = entityDict["data"] as? [String: Any] ?? [:]
            let codableData = data.mapValues { AnyCodable($0) }

            let entity = SyncEntity(type: entityType, id: entityId, data: codableData, hlc: hlc, deleted: deleted)
            onEntityReceived?(entity)
            localVector[entityId] = hlc
            saveVersionVector?(localVector)
        }

        let ack: [String: Any] = [
            "type": "live_ack",
            "change_id": changeId,
        ]
        sendJSON(ack, on: ws)
    }

    // MARK: - Peer List

    private func handlePeerList(_ json: [String: Any]) {
        guard authenticated else { return }
        guard let peersArray = json["peers"] as? [[String: Any]] else { return }

        var peers: [PeerRecord] = []
        for peerDict in peersArray {
            guard let devId = peerDict["device_id"] as? String else { continue }
            let devName = peerDict["device_name"] as? String ?? "Unknown"
            let addrs = peerDict["addresses"] as? [String] ?? []
            let lastSeen = peerDict["last_seen"] as? String ?? ""
            let lastAddr = peerDict["last_address"] as? String

            peers.append(PeerRecord(device_id: devId, device_name: devName, addresses: addrs, last_seen: lastSeen, last_address: lastAddr))
        }

        onPeerListReceived?(peers)
    }

    // MARK: - Queued Live Changes

    private func flushQueuedLiveChanges() {
        guard !queuedLiveChanges.isEmpty else { return }
        for entity in queuedLiveChanges {
            sendLiveChange(entity)
        }
        queuedLiveChanges.removeAll()
    }

    // MARK: - Reconnect

    private func scheduleReconnect() {
        guard !stopped else { return }
        guard reconnectTask == nil else { return }

        let jitter = Double.random(in: 0..<1)
        let delay = min(reconnectDelay + jitter, Self.reconnectMaxSec)
        logger.info("Reconnecting to \(self.peer.device_name) in \(Int(delay))s")

        state = .disconnected

        reconnectTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay))
            guard !Task.isCancelled else { return }
            await MainActor.run { [weak self] in
                guard let self, !self.stopped else { return }
                self.reconnectTask = nil
                self.reconnectDelay = min(self.reconnectDelay * 1.5, Self.reconnectMaxSec)
                self.connect()
            }
        }
    }

    // MARK: - Helpers

    private func entityToDict(_ entity: SyncEntity) -> [String: Any] {
        var dict: [String: Any] = [
            "type": entity.type,
            "id": entity.id,
            "data": entity.data.mapValues { $0.value },
            "hlc": entity.hlc,
        ]
        if entity.deleted == true {
            dict["deleted"] = true
        }
        return dict
    }

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
