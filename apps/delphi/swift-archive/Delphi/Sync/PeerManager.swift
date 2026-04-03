import Foundation
import SwiftData
import os

/// Coordinates SyncServer + SyncClient connections for equal-peer mesh sync.
///
/// Each device runs one PeerManager which:
/// - Starts a SyncServer on port 21531
/// - Creates SyncClient instances for each known peer
/// - Merges peer lists from all connections
/// - Persists known peers and version vector in UserDefaults
/// - Bridges incoming sync entities to SwiftData via ArkEventMapper
@Observable
@MainActor
final class PeerManager {

    // MARK: - Published State

    var connectedPeerCount: Int = 0
    var connectedPeerNames: [String] = []
    var isRunning = false

    // MARK: - Config

    private let deviceId: String
    private let deviceName: String
    private var spaceId: String = ""
    private var spaceCode: String = ""

    // MARK: - Components

    private let server = SyncServer()
    private var clients: [String: SyncClient] = [:]  // peer device_id -> client

    // MARK: - Persistence

    private var knownPeers: [PeerRecord] = []
    private var versionVector: VersionVector = [:]
    private let defaults = UserDefaults.standard
    private var modelContainer: ModelContainer?

    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "PeerManager")

    // MARK: - UserDefaults Keys

    private enum Keys {
        static let knownPeers = "sync.peers"
        static let versionVector = "lan_sync.version_vector"
    }

    // MARK: - Init

    init(deviceId: String, deviceName: String) {
        self.deviceId = deviceId
        self.deviceName = deviceName
    }

    func setModelContainer(_ container: ModelContainer) {
        self.modelContainer = container
    }

    // MARK: - Public API

    func start(spaceCode: String) {
        guard !isRunning else { return }

        self.spaceCode = spaceCode
        self.spaceId = SpaceManager().deriveSpaceId(from: spaceCode)

        // Load persisted state
        loadPersistedPeers()
        loadPersistedVersionVector()

        let ownAddresses = getOwnAddresses()
        logger.info("Own addresses: \(ownAddresses)")

        // Wire up server callbacks
        server.loadVersionVector = { [weak self] in
            self?.versionVector ?? [:]
        }
        server.saveVersionVector = { [weak self] vector in
            self?.versionVector = vector
            self?.persistVersionVector()
        }
        server.loadAllEntities = { [weak self] in
            self?.buildAllEntities() ?? []
        }
        server.onEntityReceived = { [weak self] entity in
            self?.applyIncomingEntity(entity)
        }
        server.onPeerConnected = { [weak self] deviceId, deviceName in
            self?.handlePeerConnected(deviceId, name: deviceName)
        }
        server.onPeerDisconnected = { [weak self] deviceId in
            self?.handlePeerDisconnected(deviceId)
        }
        server.onNewPeerDiscovered = { [weak self] peer in
            self?.connectToPeerIfNeeded(peer)
        }

        // Start server
        server.start(
            spaceId: spaceId,
            deviceId: deviceId,
            deviceName: deviceName,
            ownAddresses: ownAddresses,
            knownPeers: knownPeers
        )

        // Connect to all known peers
        for peer in knownPeers {
            connectToPeerIfNeeded(peer)
        }

        isRunning = true
        logger.info("PeerManager started with \(self.knownPeers.count) known peers")
    }

    func stop() {
        isRunning = false

        // Stop all clients
        for (_, client) in clients {
            client.stop()
        }
        clients.removeAll()

        // Stop server
        server.stop()

        connectedPeerCount = 0
        connectedPeerNames.removeAll()

        logger.info("PeerManager stopped")
    }

    // MARK: - Broadcast Local Changes

    /// Broadcast a change to all connected peers (via server + all clients).
    func broadcastChange(entityType: String, entityId: String, data: [String: Any], deleted: Bool = false) {
        let hlc = generateHlcString(deviceId: deviceId)
        versionVector[entityId] = hlc
        persistVersionVector()

        let codableData = data.mapValues { AnyCodable($0) }
        let entity = SyncEntity(type: entityType, id: entityId, data: codableData, hlc: hlc, deleted: deleted ? true : nil)

        // Broadcast via server to all inbound peers
        server.broadcastLiveChange(entity)

        // Send via each client to outbound peers
        for (_, client) in clients {
            if client.isConnected {
                client.sendLiveChange(entity)
            }
        }
    }

    /// Convenience: broadcast a TodoItem change.
    func broadcastTodoChange(_ todo: TodoItem) {
        let event = ArkEventMapper.todoToArkEvent(todo)
        let innerData = event["data"] as? [String: Any] ?? event
        broadcastChange(entityType: "todo", entityId: todo.id.uuidString.lowercased(), data: innerData)
    }

    /// Convenience: broadcast a TodoItem deletion.
    func broadcastTodoDelete(_ id: UUID) {
        broadcastChange(entityType: "todo", entityId: id.uuidString.lowercased(), data: [:], deleted: true)
    }

    /// Convenience: broadcast a Project change.
    func broadcastProjectChange(_ project: Project) {
        let event = ArkEventMapper.projectToArkEvent(project)
        let innerData = event["data"] as? [String: Any] ?? event
        broadcastChange(entityType: "project", entityId: project.id.uuidString.lowercased(), data: innerData)
    }

    /// Convenience: broadcast a Project deletion.
    func broadcastProjectDelete(_ id: UUID) {
        broadcastChange(entityType: "project", entityId: id.uuidString.lowercased(), data: [:], deleted: true)
    }

    /// Convenience: broadcast an Area change.
    func broadcastAreaChange(_ area: Area) {
        let event = ArkEventMapper.areaToArkEvent(area)
        let innerData = event["data"] as? [String: Any] ?? event
        broadcastChange(entityType: "area", entityId: area.id.uuidString.lowercased(), data: innerData)
    }

    /// Convenience: broadcast a Tag change.
    func broadcastTagChange(_ tag: Tag) {
        let event = ArkEventMapper.tagToArkEvent(tag)
        let innerData = event["data"] as? [String: Any] ?? event
        broadcastChange(entityType: "tag", entityId: tag.id.uuidString.lowercased(), data: innerData)
    }

    /// Convenience: broadcast a Heading change.
    func broadcastHeadingChange(_ heading: Heading) {
        var data: [String: Any] = [
            "title": heading.title,
            "sortOrder": heading.sortOrder,
        ]
        if let project = heading.project {
            data["projectId"] = project.id.uuidString.lowercased()
        }
        broadcastChange(entityType: "heading", entityId: heading.id.uuidString.lowercased(), data: data)
    }

    /// Convenience: broadcast a Heading deletion.
    func broadcastHeadingDelete(_ id: UUID) {
        broadcastChange(entityType: "heading", entityId: id.uuidString.lowercased(), data: [:], deleted: true)
    }

    // MARK: - Add Peer (e.g. from QR scan)

    /// Add a peer and connect to it. Used after joining a space via QR/code.
    func addPeerAndConnect(_ peer: PeerRecord) {
        knownPeers = mergePeerRecords(knownPeers, [peer])
        persistPeers()
        server.updateKnownPeers(knownPeers)
        connectToPeerIfNeeded(peer)
    }

    // MARK: - Internal: Connect to Peer

    private func connectToPeerIfNeeded(_ peer: PeerRecord) {
        // Skip self
        guard peer.device_id != deviceId else { return }
        // Skip if already connected via client
        guard clients[peer.device_id] == nil else { return }
        // Skip if server has inbound connection
        guard !server.isConnectedTo(peer.device_id) else { return }

        let ownAddresses = getOwnAddresses()

        let client = SyncClient(
            peer: peer,
            spaceId: spaceId,
            deviceId: deviceId,
            deviceName: deviceName,
            ownAddresses: ownAddresses
        )

        // Wire up callbacks
        client.loadVersionVector = { [weak self] in
            self?.versionVector ?? [:]
        }
        client.saveVersionVector = { [weak self] vector in
            self?.versionVector = vector
            self?.persistVersionVector()
        }
        client.loadAllEntities = { [weak self] in
            self?.buildAllEntities() ?? []
        }
        client.onEntityReceived = { [weak self] entity in
            self?.applyIncomingEntity(entity)
        }
        client.onPeerConnected = { [weak self] deviceId, deviceName in
            self?.handlePeerConnected(deviceId, name: deviceName)

            // Register with server so it knows about this peer
            if let client = self?.clients[deviceId] {
                self?.server.registerExternalPeer(deviceId, deviceName: deviceName, addresses: client.peer.addresses)
            }
        }
        client.onPeerDisconnected = { [weak self] deviceId in
            self?.handlePeerDisconnected(deviceId)
        }
        client.onPeerListReceived = { [weak self] peers in
            self?.handlePeerListFromClient(peers)
        }

        clients[peer.device_id] = client
        client.start()
    }

    // MARK: - Peer Events

    private func handlePeerConnected(_ peerId: String, name: String) {
        // Update known peers
        let record = PeerRecord(
            device_id: peerId,
            device_name: name,
            addresses: clients[peerId]?.peer.addresses ?? [],
            last_seen: ISO8601DateFormatter().string(from: Date())
        )
        knownPeers = mergePeerRecords(knownPeers, [record])
        persistPeers()
        server.updateKnownPeers(knownPeers)
        updateConnectedState()
    }

    private func handlePeerDisconnected(_ peerId: String) {
        updateConnectedState()
    }

    private func handlePeerListFromClient(_ peers: [PeerRecord]) {
        let incoming = peers.filter { $0.device_id != deviceId }
        let beforeCount = knownPeers.count
        knownPeers = mergePeerRecords(knownPeers, incoming)
        persistPeers()
        server.updateKnownPeers(knownPeers)

        // Try connecting to newly discovered peers
        for peer in incoming {
            if clients[peer.device_id] == nil && !server.isConnectedTo(peer.device_id) {
                connectToPeerIfNeeded(peer)
            }
        }

        if knownPeers.count > beforeCount {
            logger.info("Peer list from client: \(beforeCount) -> \(self.knownPeers.count) known peers")
        }
    }

    private func updateConnectedState() {
        let serverPeers = Set(server.connectedPeerIds)
        let clientPeers = Set(clients.filter { $0.value.isConnected }.keys)
        let allConnected = serverPeers.union(clientPeers)
        connectedPeerCount = allConnected.count
        connectedPeerNames = allConnected.compactMap { peerId in
            if let name = knownPeers.first(where: { $0.device_id == peerId })?.device_name {
                return name
            }
            return peerId
        }
    }

    // MARK: - Apply Incoming Entity to SwiftData

    private func applyIncomingEntity(_ entity: SyncEntity) {
        guard let container = modelContainer else { return }
        let context = container.mainContext
        let data = entity.data.mapValues { $0.value }

        if entity.deleted == true {
            guard let uuid = UUID(uuidString: entity.id) else { return }
            switch entity.type {
            case "todo":
                let descriptor = FetchDescriptor<TodoItem>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first { context.delete(item) }
            case "project":
                let descriptor = FetchDescriptor<Project>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first { context.delete(item) }
            case "heading":
                let descriptor = FetchDescriptor<Heading>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first { context.delete(item) }
            case "area":
                let descriptor = FetchDescriptor<Area>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first { context.delete(item) }
            case "tag":
                let descriptor = FetchDescriptor<Tag>(predicate: #Predicate { $0.id == uuid })
                if let item = try? context.fetch(descriptor).first { context.delete(item) }
            default:
                break
            }
            try? context.save()
            return
        }

        switch entity.type {
        case "todo":
            let event: [String: Any] = ["source_id": entity.id, "data": data, "event_type": "task"]
            _ = ArkEventMapper.arkEventToTodo(event, context: context)
        case "project":
            let event: [String: Any] = ["source_id": entity.id, "data": data, "event_type": "project"]
            _ = ArkEventMapper.arkEventToProject(event, context: context)
        case "area":
            let event: [String: Any] = ["source_id": entity.id, "data": data, "event_type": "area"]
            _ = ArkEventMapper.arkEventToArea(event, context: context)
        case "tag":
            let event: [String: Any] = ["source_id": entity.id, "data": data, "event_type": "tag"]
            _ = ArkEventMapper.arkEventToTag(event, context: context)
        case "heading":
            if let title = data["title"] as? String, let uuid = UUID(uuidString: entity.id) {
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
            }
        default:
            break
        }
        try? context.save()
    }

    // MARK: - Build All Entities from SwiftData

    private func buildAllEntities() -> [SyncEntity] {
        guard let container = modelContainer else { return [] }
        let context = container.mainContext

        var entities: [SyncEntity] = []

        let todos = (try? context.fetch(FetchDescriptor<TodoItem>())) ?? []
        let projects = (try? context.fetch(FetchDescriptor<Project>())) ?? []
        let areas = (try? context.fetch(FetchDescriptor<Area>())) ?? []
        let tags = (try? context.fetch(FetchDescriptor<Tag>())) ?? []
        let headings = (try? context.fetch(FetchDescriptor<Heading>())) ?? []

        for todo in todos {
            let id = todo.id.uuidString.lowercased()
            let event = ArkEventMapper.todoToArkEvent(todo)
            let innerData = event["data"] as? [String: Any] ?? event
            let hlc = versionVector[id] ?? generateHlcString(deviceId: deviceId)
            if versionVector[id] == nil {
                versionVector[id] = hlc
            }
            entities.append(SyncEntity(
                type: "todo", id: id,
                data: innerData.mapValues { AnyCodable($0) },
                hlc: hlc
            ))
        }

        for project in projects {
            let id = project.id.uuidString.lowercased()
            let event = ArkEventMapper.projectToArkEvent(project)
            let innerData = event["data"] as? [String: Any] ?? event
            let hlc = versionVector[id] ?? generateHlcString(deviceId: deviceId)
            if versionVector[id] == nil {
                versionVector[id] = hlc
            }
            entities.append(SyncEntity(
                type: "project", id: id,
                data: innerData.mapValues { AnyCodable($0) },
                hlc: hlc
            ))
        }

        for area in areas {
            let id = area.id.uuidString.lowercased()
            let event = ArkEventMapper.areaToArkEvent(area)
            let innerData = event["data"] as? [String: Any] ?? event
            let hlc = versionVector[id] ?? generateHlcString(deviceId: deviceId)
            if versionVector[id] == nil {
                versionVector[id] = hlc
            }
            entities.append(SyncEntity(
                type: "area", id: id,
                data: innerData.mapValues { AnyCodable($0) },
                hlc: hlc
            ))
        }

        for tag in tags {
            let id = tag.id.uuidString.lowercased()
            let event = ArkEventMapper.tagToArkEvent(tag)
            let innerData = event["data"] as? [String: Any] ?? event
            let hlc = versionVector[id] ?? generateHlcString(deviceId: deviceId)
            if versionVector[id] == nil {
                versionVector[id] = hlc
            }
            entities.append(SyncEntity(
                type: "tag", id: id,
                data: innerData.mapValues { AnyCodable($0) },
                hlc: hlc
            ))
        }

        for heading in headings {
            let id = heading.id.uuidString.lowercased()
            var headingData: [String: Any] = ["title": heading.title, "sortOrder": heading.sortOrder]
            if let project = heading.project {
                headingData["projectId"] = project.id.uuidString.lowercased()
            }
            let hlc = versionVector[id] ?? generateHlcString(deviceId: deviceId)
            if versionVector[id] == nil {
                versionVector[id] = hlc
            }
            entities.append(SyncEntity(
                type: "heading", id: id,
                data: headingData.mapValues { AnyCodable($0) },
                hlc: hlc
            ))
        }

        persistVersionVector()
        return entities
    }

    // MARK: - Persistence

    private func loadPersistedPeers() {
        guard let raw = defaults.string(forKey: Keys.knownPeers),
              let data = raw.data(using: .utf8),
              let parsed = try? JSONDecoder().decode([PeerRecord].self, from: data)
        else {
            knownPeers = []
            return
        }
        knownPeers = parsed
        logger.info("Loaded \(self.knownPeers.count) persisted peers")
    }

    private func persistPeers() {
        guard let data = try? JSONEncoder().encode(knownPeers),
              let string = String(data: data, encoding: .utf8)
        else { return }
        defaults.set(string, forKey: Keys.knownPeers)
    }

    private func loadPersistedVersionVector() {
        guard let raw = defaults.string(forKey: Keys.versionVector),
              let data = raw.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: String]
        else {
            versionVector = [:]
            return
        }
        versionVector = dict
    }

    private func persistVersionVector() {
        guard let data = try? JSONSerialization.data(withJSONObject: versionVector),
              let string = String(data: data, encoding: .utf8)
        else { return }
        defaults.set(string, forKey: Keys.versionVector)
    }

    // MARK: - Server helper: register external peer

    private func registerExternalPeer(_ deviceId: String, deviceName: String, addresses: [String]) {
        server.registerExternalPeer(deviceId, deviceName: deviceName, addresses: addresses)
    }
}

// MARK: - SyncServer extension for external peer registration

extension SyncServer {
    /// Register an externally-managed peer connection (from SyncClient outbound).
    func registerExternalPeer(_ deviceId: String, deviceName: String, addresses: [String]) {
        let record = PeerRecord(
            device_id: deviceId,
            device_name: deviceName,
            addresses: addresses,
            last_seen: ISO8601DateFormatter().string(from: Date())
        )
        updateKnownPeers(mergePeerRecords([record], []))
    }
}
