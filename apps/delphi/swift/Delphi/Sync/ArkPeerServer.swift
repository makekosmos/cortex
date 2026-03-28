import Foundation
import Network
import os

/// WebSocket server using Network.framework NWListener.
///
/// Accepts inbound P2P connections from LAN peers, authenticates them
/// via the mesh secret HMAC handshake, and relays changes.
@Observable
@MainActor
final class ArkPeerServer {

    // MARK: - State

    var connectedPeerIds: [String] = []

    // MARK: - Config

    private let meshSecret: String
    private let deviceId: String
    private let deviceName: String
    private let port: UInt16

    // MARK: - Internal

    private var listener: NWListener?
    private var connections: [String: NWConnection] = [:]  // peer deviceId -> connection
    private var pendingConnections: [ObjectIdentifier: NWConnection] = [:]
    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "PeerServer")

    /// Called when a change is received from a peer.
    var onPeerChange: (([String: Any], String) -> Void)?  // (change, fromDeviceId)

    // MARK: - Init

    init(meshSecret: String, deviceId: String, deviceName: String, port: UInt16 = 9473) {
        self.meshSecret = meshSecret
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.port = port
    }

    // MARK: - Lifecycle

    /// Start the WebSocket server. Optionally advertise via Bonjour with TXT metadata.
    func start(advertiseTXT: NWTXTRecord? = nil) throws {
        guard listener == nil else { return }

        let params = NWParameters.tcp
        let wsOptions = NWProtocolWebSocket.Options()
        params.defaultProtocolStack.applicationProtocols.insert(wsOptions, at: 0)

        let nwListener = try NWListener(using: params, on: NWEndpoint.Port(rawValue: port)!)
        self.listener = nwListener

        // Optionally register a Bonjour service on this listener
        if let txt = advertiseTXT {
            nwListener.service = NWListener.Service(name: deviceId, type: "_ark-peer._tcp.", txtRecord: txt)
        }

        nwListener.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                guard let self else { return }
                switch state {
                case .ready:
                    if let port = nwListener.port {
                        self.logger.info("PeerServer listening on port \(port.rawValue)")
                    }
                case .failed(let error):
                    self.logger.error("PeerServer listener failed: \(error.localizedDescription)")
                    self.stop()
                case .cancelled:
                    self.logger.info("PeerServer listener cancelled")
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
        connectedPeerIds.removeAll()

        listener?.cancel()
        listener = nil
        logger.info("PeerServer stopped")
    }

    var actualPort: UInt16 {
        listener?.port?.rawValue ?? port
    }

    // MARK: - Broadcast

    /// Send a change to all connected inbound peers (with loop prevention via hop_path).
    func broadcastChange(_ message: Data, excludeDevice: String? = nil, hopPath: [String] = []) {
        for (peerDeviceId, connection) in connections {
            if peerDeviceId == excludeDevice { continue }
            if hopPath.contains(peerDeviceId) { continue }

            let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
            let context = NWConnection.ContentContext(identifier: "change", metadata: [metadata])
            connection.send(content: message, contentContext: context, isComplete: true, completion: .contentProcessed { [weak self] error in
                if let error {
                    self?.logger.error("Failed to send to \(peerDeviceId): \(error.localizedDescription)")
                }
            })
        }
    }

    // MARK: - Connection handling

    private func handleNewConnection(_ connection: NWConnection) {
        pendingConnections[ObjectIdentifier(connection)] = connection
        logger.info("New inbound connection")

        connection.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                guard let self else { return }
                switch state {
                case .ready:
                    self.logger.info("Inbound connection ready, waiting for peer_hello")
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
                  type == "peer_hello"
            else {
                self.logger.warning("Expected peer_hello, closing connection")
                connection.cancel()
                self.pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
                return
            }

            self.handleHello(json, connection: connection)
        }
    }

    private func handleHello(_ json: [String: Any], connection: NWConnection) {
        guard let peerDeviceId = json["device_id"] as? String,
              let peerDeviceName = json["device_name"] as? String,
              let meshId = json["mesh_id"] as? String,
              let nonce = json["nonce"] as? String,
              let authHmac = json["auth_hmac"] as? String
        else {
            sendAckAndClose(ok: false, error: "invalid peer_hello", connection: connection)
            return
        }

        // Verify mesh ID
        let expectedMeshId = PeerProtocol.computeMeshId(meshSecret: meshSecret)
        guard meshId == expectedMeshId else {
            logger.warning("Mesh ID mismatch from \(peerDeviceName)")
            sendAckAndClose(ok: false, error: "mesh_id mismatch", connection: connection)
            return
        }

        // Verify HMAC
        guard PeerProtocol.verifyAuthHmac(meshSecret: meshSecret, nonce: nonce, provided: authHmac) else {
            logger.warning("Auth HMAC verification failed from \(peerDeviceName)")
            sendAckAndClose(ok: false, error: "auth failed", connection: connection)
            return
        }

        // Prevent self-connection
        guard peerDeviceId != deviceId else {
            logger.info("Rejecting self-connection")
            connection.cancel()
            pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
            return
        }

        // Close existing connection from the same device
        if let existing = connections[peerDeviceId] {
            existing.cancel()
            connections.removeValue(forKey: peerDeviceId)
        }

        // Promote from pending to authenticated
        pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
        connections[peerDeviceId] = connection
        connectedPeerIds = Array(connections.keys)
        logger.info("Authenticated peer: \(peerDeviceId) (\(peerDeviceName))")

        // Send success ack
        let ack = PeerHelloAck.create(
            ok: true,
            deviceId: deviceId,
            deviceName: deviceName,
            platform: "macos",
            meshSecret: meshSecret
        )
        sendJSON(ack, on: connection)

        // Enter receive loop
        startReceiveLoop(connection, peerDeviceId: peerDeviceId)
    }

    private func startReceiveLoop(_ connection: NWConnection, peerDeviceId: String) {
        receiveWSMessage(connection) { [weak self] data in
            guard let self else { return }
            guard let data else {
                // Connection closed
                self.logger.info("Peer \(peerDeviceId) disconnected")
                self.connections.removeValue(forKey: peerDeviceId)
                self.connectedPeerIds = Array(self.connections.keys)
                return
            }

            // Parse as change
            if let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let type = json["type"] as? String,
               type == "change" {
                // Add ourselves to hop_path
                var hopPath = json["hop_path"] as? [String] ?? []
                if !hopPath.contains(self.deviceId) {
                    hopPath.append(self.deviceId)
                }

                // Notify handler
                self.onPeerChange?(json, peerDeviceId)

                // Re-broadcast to other inbound peers
                var forwarded = json
                forwarded["hop_path"] = hopPath
                if let forwardData = try? JSONSerialization.data(withJSONObject: forwarded) {
                    self.broadcastChange(forwardData, excludeDevice: peerDeviceId, hopPath: hopPath)
                }
            }

            // Continue receiving
            self.startReceiveLoop(connection, peerDeviceId: peerDeviceId)
        }
    }

    // MARK: - WebSocket I/O helpers

    private func receiveWSMessage(_ connection: NWConnection, completion: @escaping @MainActor (Data?) -> Void) {
        connection.receiveMessage { content, context, isComplete, error in
            Task { @MainActor in
                if let error {
                    self.logger.error("Receive error: \(error.localizedDescription)")
                    completion(nil)
                    return
                }
                completion(content)
            }
        }
    }

    private func sendJSON<T: Encodable>(_ value: T, on connection: NWConnection) {
        guard let data = try? JSONEncoder().encode(value) else { return }
        let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
        let context = NWConnection.ContentContext(identifier: "msg", metadata: [metadata])
        connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { [weak self] error in
            if let error {
                self?.logger.error("Send error: \(error.localizedDescription)")
            }
        })
    }

    private func sendAckAndClose(ok: Bool, error: String, connection: NWConnection) {
        let ack = PeerHelloAck.create(
            ok: ok,
            deviceId: deviceId,
            deviceName: deviceName,
            platform: "macos",
            meshSecret: meshSecret,
            error: error
        )
        sendJSON(ack, on: connection)

        // Give the message time to send before closing
        Task {
            try? await Task.sleep(for: .milliseconds(100))
            connection.cancel()
            await MainActor.run { [weak self] in
                self?.pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
            }
        }
    }

    private func cleanupConnection(_ connection: NWConnection) {
        pendingConnections.removeValue(forKey: ObjectIdentifier(connection))
        // Find and remove from authenticated connections
        if let deviceId = connections.first(where: { $0.value === connection })?.key {
            connections.removeValue(forKey: deviceId)
            connectedPeerIds = Array(connections.keys)
            logger.info("Peer \(deviceId) connection cleaned up")
        }
    }
}
