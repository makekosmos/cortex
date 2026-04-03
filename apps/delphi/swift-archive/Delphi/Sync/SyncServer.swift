import Foundation
import Network
import os

/// WebSocket server for LAN sync using Network.framework NWListener.
///
/// Equal-peer model: every device runs both a WS server and WS client.
/// This is the server half -- accepts incoming connections.
///
/// Protocol flow per connection:
///   1. Client sends `hello` (with addresses[]) -> server validates, replies with `hello`
///   2. Both exchange `version_vector` messages
///   3. Both exchange `peer_list` messages
///   4. Both exchange `sync_changes` batches with ACKs
///   5. Enter live mode: mutations broadcast as `live_change` with `live_ack`
@Observable
@MainActor
final class SyncServer {

    // MARK: - State

    var isRunning = false

    // MARK: - Config

    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""
    private var ownAddresses: [String] = []

    // MARK: - Callbacks

    var onEntityReceived: ((SyncEntity) -> Void)?
    var onPeerConnected: ((String, String) -> Void)?   // (deviceId, deviceName)
    var onPeerDisconnected: ((String) -> Void)?
    var onNewPeerDiscovered: ((PeerRecord) -> Void)?

    // MARK: - Internal

    private var listener: NWListener?
    private var connections: [String: NWConnection] = [:]  // peer deviceId -> connection
    private var peerNames: [String: String] = [:]          // peer deviceId -> deviceName
    private var pendingConnections: [ObjectIdentifier: NWConnection] = [:]
    private var peerSyncComplete: Set<String> = []
    private var peerVersionVectors: [String: VersionVector] = [:]
    private var knownPeerRecords: [PeerRecord] = []
    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "SyncServer")

    /// Version vector: entity_id -> HLC string. Loaded from delegate (PeerManager).
    var loadVersionVector: (() -> VersionVector)?
    var saveVersionVector: ((VersionVector) -> Void)?
    var loadAllEntities: (() -> [SyncEntity])?

    // MARK: - Public API

    var connectedPeerIds: [String] { Array(connections.keys) }
    var connectedPeerCount: Int { connections.count }

    func start(spaceId: String, deviceId: String, deviceName: String, ownAddresses: [String], knownPeers: [PeerRecord]) {
        guard !isRunning else { return }

        self.spaceId = spaceId
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.ownAddresses = ownAddresses
        self.knownPeerRecords = knownPeers

        let params = NWParameters.tcp
        let wsOptions = NWProtocolWebSocket.Options()
        params.defaultProtocolStack.applicationProtocols.insert(wsOptions, at: 0)

        do {
            let nwListener = try NWListener(using: params, on: NWEndpoint.Port(rawValue: LAN_SYNC_PORT)!)
            self.listener = nwListener

            nwListener.stateUpdateHandler = { [weak self] state in
                Task { @MainActor in
                    guard let self else { return }
                    switch state {
                    case .ready:
                        if let port = nwListener.port {
                            self.logger.info("SyncServer listening on port \(port.rawValue)")
                        }
                    case .failed(let error):
                        self.logger.error("SyncServer listener failed: \(error.localizedDescription)")
                        self.stop()
                    default:
                        break
                    }
                }
            }

            nwListener.newConnectionHandler = { [weak self] connection in
                Task { @MainActor in
                    self?.handleNewConnection(connection)
                }
            }

            nwListener.start(queue: .main)
            isRunning = true
            logger.info("SyncServer started on port \(LAN_SYNC_PORT)")
        } catch {
            logger.error("Failed to start SyncServer: \(error.localizedDescription)")
        }
    }

    func stop() {
        for (_, connection) in pendingConnections {
            connection.cancel()
        }
        pendingConnections.removeAll()

        for (_, connection) in connections {
            connection.cancel()
        }
        connections.removeAll()
        peerNames.removeAll()
        peerSyncComplete.removeAll()
        peerVersionVectors.removeAll()

        listener?.cancel()
        listener = nil
        isRunning = false
        logger.info("SyncServer stopped")
    }

    func updateOwnAddresses(_ addresses: [String]) {
        self.ownAddresses = addresses
    }

    func updateKnownPeers(_ peers: [PeerRecord]) {
        self.knownPeerRecords = peers
    }

    // MARK: - Broadcast live change to all connected peers

    func broadcastLiveChange(_ entity: SyncEntity, excludeDeviceId: String? = nil) {
        let changeId = generateSyncId()
        let msg: [String: Any] = [
            "type": "live_change",
            "change_id": changeId,
            "entity": entityToDict(entity),
        ]

        for (peerId, connection) in connections {
            if peerId == excludeDeviceId { continue }
            sendJSON(msg, on: connection)
        }
    }

    /// Check if a specific peer is connected to this server.
    func isConnectedTo(_ deviceId: String) -> Bool {
        connections[deviceId] != nil
    }

    // MARK: - Connection Handling

    private func handleNewConnection(_ connection: NWConnection) {
        pendingConnections[ObjectIdentifier(connection)] = connection
        logger.info("New inbound connection")

        connection.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                guard let self else { return }
                switch state {
                case .ready:
                    self.receiveHandshake(connection)
                case .failed(let error):
                    self.logger.error("Inbound connection failed: \(error.localizedDescription)")
                    self.cleanupConnection(connection)
                case .cancelled:
                    self.cleanupConnection(connection)
                default:
                    break
                }
            }
        }

        connection.start(queue: .main)

        // Handshake timeout
        Task {
            try? await Task.sleep(for: .seconds(10))
            await MainActor.run { [weak self] in
                guard let self else { return }
                if self.pendingConnections[ObjectIdentifier(connection)] != nil {
                    self.logger.warning("Handshake timeout, closing connection")
                    connection.cancel()
                    self.pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
                }
            }
        }
    }

    private func receiveHandshake(_ connection: NWConnection) {
        receiveWSMessage(connection) { [weak self] data in
            guard let self else { return }
            guard let data,
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let type = json["type"] as? String,
                  type == "hello"
            else {
                self.logger.warning("Expected hello, closing connection")
                connection.cancel()
                self.pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
                return
            }
            self.handleHello(json, connection: connection)
        }
    }

    private func handleHello(_ json: [String: Any], connection: NWConnection) {
        guard let version = json["protocol_version"] as? Int,
              version == SYNC_PROTOCOL_VERSION
        else {
            logger.warning("Protocol version mismatch")
            sendJSONAndClose(["type": "error", "message": "protocol version mismatch"], connection: connection)
            return
        }

        guard let peerDeviceId = json["device_id"] as? String,
              let peerDeviceName = json["device_name"] as? String
        else {
            logger.warning("Invalid hello: missing device_id or device_name")
            connection.cancel()
            pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
            return
        }

        // Prevent self-connection
        guard peerDeviceId != deviceId else {
            logger.info("Rejecting self-connection")
            connection.cancel()
            pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
            return
        }

        let peerAddresses = json["addresses"] as? [String] ?? []

        // Close existing connection from the same device
        if let existing = connections[peerDeviceId] {
            existing.cancel()
            connections.removeValue(forKey: peerDeviceId)
        }

        // Promote from pending to authenticated
        pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
        connections[peerDeviceId] = connection
        peerNames[peerDeviceId] = peerDeviceName
        logger.info("Authenticated peer: \(peerDeviceName) (\(peerDeviceId)), addresses: \(peerAddresses)")

        // Update peer record with announced addresses
        if !peerAddresses.isEmpty {
            let record = PeerRecord(
                device_id: peerDeviceId,
                device_name: peerDeviceName,
                addresses: peerAddresses,
                last_seen: ISO8601DateFormatter().string(from: Date())
            )
            knownPeerRecords = mergePeerRecords(knownPeerRecords, [record])
        }

        // Send our hello back
        let helloBack: [String: Any] = [
            "type": "hello",
            "protocol_version": SYNC_PROTOCOL_VERSION,
            "device_id": deviceId,
            "device_name": deviceName,
            "space_id": spaceId,
            "addresses": ownAddresses,
        ]
        sendJSON(helloBack, on: connection)

        onPeerConnected?(peerDeviceId, peerDeviceName)

        // Send our version vector
        sendVersionVector(to: connection, peerId: peerDeviceId)

        // Send peer list after a short delay
        Task {
            try? await Task.sleep(for: .milliseconds(100))
            await MainActor.run { [weak self] in
                self?.sendPeerList(to: connection)
            }
        }

        // Start receive loop
        startReceiveLoop(connection, peerDeviceId: peerDeviceId)
    }

    // MARK: - Receive Loop

    private func startReceiveLoop(_ connection: NWConnection, peerDeviceId: String) {
        receiveWSMessage(connection) { [weak self] data in
            guard let self else { return }
            guard let data else {
                self.logger.info("Peer \(peerDeviceId) disconnected")
                self.removePeer(peerDeviceId)
                return
            }

            guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let type = json["type"] as? String
            else {
                self.startReceiveLoop(connection, peerDeviceId: peerDeviceId)
                return
            }

            self.handleMessage(json, type: type, connection: connection, peerDeviceId: peerDeviceId)
            self.startReceiveLoop(connection, peerDeviceId: peerDeviceId)
        }
    }

    private func handleMessage(_ json: [String: Any], type: String, connection: NWConnection, peerDeviceId: String) {
        switch type {
        case "version_vector":
            handleVersionVector(json, connection: connection, peerDeviceId: peerDeviceId)
        case "sync_changes":
            handleSyncChanges(json, connection: connection, peerDeviceId: peerDeviceId)
        case "sync_ack":
            break // ACK for our batches
        case "live_change":
            handleLiveChange(json, connection: connection, peerDeviceId: peerDeviceId)
        case "live_ack":
            break
        case "peer_list":
            handlePeerList(json, peerDeviceId: peerDeviceId)
        case "ping":
            sendJSON(["type": "pong", "ts": json["ts"] ?? 0], on: connection)
        case "pong":
            break
        default:
            break
        }
    }

    // MARK: - Version Vector Exchange

    private func sendVersionVector(to connection: NWConnection, peerId: String) {
        var vector = loadVersionVector?() ?? [:]

        if vector.isEmpty {
            // Build from DB
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
        sendJSON(msg, on: connection)
    }

    private func handleVersionVector(_ json: [String: Any], connection: NWConnection, peerDeviceId: String) {
        guard let remoteVector = json["vector"] as? [String: String] else { return }
        peerVersionVectors[peerDeviceId] = remoteVector

        var localVector = loadVersionVector?() ?? [:]
        let allEntities = loadAllEntities?() ?? []

        // Update vector for entities that lack an entry
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
            logger.info("Sending \(entitiesToSend.count) entities to \(peerDeviceId)")
            sendBatches(entitiesToSend, to: connection)
        } else {
            // Send empty final batch
            let msg: [String: Any] = [
                "type": "sync_changes",
                "batch_id": generateSyncId(),
                "entities": [] as [[String: Any]],
                "is_last": true,
            ]
            sendJSON(msg, on: connection)
        }

        peerSyncComplete.insert(peerDeviceId)
    }

    // MARK: - Sync Batches

    private func sendBatches(_ entities: [SyncEntity], to connection: NWConnection) {
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
            sendJSON(msg, on: connection)
        }
    }

    private func handleSyncChanges(_ json: [String: Any], connection: NWConnection, peerDeviceId: String) {
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

                // Re-broadcast to other connected peers
                broadcastLiveChange(entity, excludeDeviceId: peerDeviceId)
            }
        }

        saveVersionVector?(localVector)

        let ack: [String: Any] = [
            "type": "sync_ack",
            "batch_id": batchId,
            "accepted": accepted,
        ]
        sendJSON(ack, on: connection)

        if isLast {
            peerSyncComplete.insert(peerDeviceId)
            logger.info("Received all sync batches from \(peerDeviceId)")
        }
    }

    // MARK: - Live Mode

    private func handleLiveChange(_ json: [String: Any], connection: NWConnection, peerDeviceId: String) {
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

            // Re-broadcast to other connected peers
            broadcastLiveChange(entity, excludeDeviceId: peerDeviceId)
        }

        let ack: [String: Any] = [
            "type": "live_ack",
            "change_id": changeId,
        ]
        sendJSON(ack, on: connection)
    }

    // MARK: - Peer List Exchange

    private func sendPeerList(to connection: NWConnection) {
        let peersData = knownPeerRecords.map { peer -> [String: Any] in
            var dict: [String: Any] = [
                "device_id": peer.device_id,
                "device_name": peer.device_name,
                "addresses": peer.addresses,
                "last_seen": peer.last_seen,
            ]
            if let lastAddr = peer.last_address {
                dict["last_address"] = lastAddr
            }
            return dict
        }

        let msg: [String: Any] = [
            "type": "peer_list",
            "peers": peersData,
        ]
        sendJSON(msg, on: connection)
    }

    private func handlePeerList(_ json: [String: Any], peerDeviceId: String) {
        guard let peersArray = json["peers"] as? [[String: Any]] else { return }

        var incomingPeers: [PeerRecord] = []
        for peerDict in peersArray {
            guard let devId = peerDict["device_id"] as? String,
                  devId != deviceId  // Skip self
            else { continue }
            let devName = peerDict["device_name"] as? String ?? "Unknown"
            let addrs = peerDict["addresses"] as? [String] ?? []
            let lastSeen = peerDict["last_seen"] as? String ?? ""
            let lastAddr = peerDict["last_address"] as? String

            let record = PeerRecord(device_id: devId, device_name: devName, addresses: addrs, last_seen: lastSeen, last_address: lastAddr)
            incomingPeers.append(record)
        }

        let beforeCount = knownPeerRecords.count
        knownPeerRecords = mergePeerRecords(knownPeerRecords, incomingPeers)

        // Notify about new peers to connect to
        for peer in incomingPeers {
            if peer.device_id == peerDeviceId { continue }
            if connections[peer.device_id] != nil { continue }
            if let record = knownPeerRecords.first(where: { $0.device_id == peer.device_id }) {
                onNewPeerDiscovered?(record)
            }
        }

        if knownPeerRecords.count > beforeCount {
            logger.info("Peer list updated: \(beforeCount) -> \(self.knownPeerRecords.count) known peers")
        }
    }

    // MARK: - Helpers

    private func removePeer(_ peerDeviceId: String) {
        connections.removeValue(forKey: peerDeviceId)
        peerNames.removeValue(forKey: peerDeviceId)
        peerSyncComplete.remove(peerDeviceId)
        peerVersionVectors.removeValue(forKey: peerDeviceId)
        onPeerDisconnected?(peerDeviceId)
    }

    private func cleanupConnection(_ connection: NWConnection) {
        pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
        if let deviceId = connections.first(where: { $0.value === connection })?.key {
            removePeer(deviceId)
        }
    }

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

    // MARK: - WebSocket I/O

    private func receiveWSMessage(_ connection: NWConnection, completion: @escaping @MainActor (Data?) -> Void) {
        connection.receiveMessage { content, _, _, error in
            Task { @MainActor in
                if error != nil {
                    completion(nil)
                    return
                }
                completion(content)
            }
        }
    }

    private func sendJSON(_ json: [String: Any], on connection: NWConnection) {
        guard let data = try? JSONSerialization.data(withJSONObject: json) else { return }
        let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
        let context = NWConnection.ContentContext(identifier: "msg", metadata: [metadata])
        connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { [weak self] error in
            if let error {
                self?.logger.error("Send error: \(error.localizedDescription)")
            }
        })
    }

    private func sendJSONAndClose(_ json: [String: Any], connection: NWConnection) {
        sendJSON(json, on: connection)
        Task {
            try? await Task.sleep(for: .milliseconds(100))
            connection.cancel()
            await MainActor.run { [weak self] in
                self?.pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
            }
        }
    }
}
