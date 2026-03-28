import Foundation
import Network
import os

/// Coordinates PeerServer + mDNS discovery + outbound WebSocket connections.
///
/// Lifecycle:
///   1. Start PeerServer on port 9473
///   2. Advertise `_ark-peer._tcp.` via NWListener (Bonjour)
///   3. Browse for peers with the same mesh_id
///   4. Connect to discovered peers via URLSessionWebSocketTask
///   5. Relay changes between all connections
@Observable
@MainActor
final class ArkPeerManager {

    // MARK: - Published state

    var connectedPeers: [String] = []
    var isRunning = false

    // MARK: - Dependencies

    let server: ArkPeerServer
    private let meshSecret: String
    private let meshId: String
    private let deviceId: String
    private let deviceName: String
    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "PeerManager")

    // MARK: - mDNS

    private var browser: NWBrowser?

    // MARK: - Outbound connections

    private var outbound: [String: URLSessionWebSocketTask] = [:]  // peer deviceId -> ws
    private var outboundReceiveTasks: [String: Task<Void, Never>] = [:]
    private var reconnectTasks: [String: Task<Void, Never>] = [:]
    private var session: URLSession?
    private var stopped = true

    /// Called when a change arrives from any peer (inbound or outbound).
    var onPeerChange: (([String: Any], String) -> Void)?

    // MARK: - Init

    init(meshSecret: String, deviceId: String, deviceName: String, port: UInt16 = 9473) {
        self.meshSecret = meshSecret
        self.meshId = PeerProtocol.computeMeshId(meshSecret: meshSecret)
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.server = ArkPeerServer(meshSecret: meshSecret, deviceId: deviceId, deviceName: deviceName, port: port)

        // Forward inbound changes from server
        self.server.onPeerChange = { [weak self] change, fromDevice in
            Task { @MainActor in
                self?.handleIncomingChange(change, fromDevice: fromDevice)
            }
        }
    }

    // MARK: - Lifecycle

    func start() {
        guard !isRunning else { return }
        stopped = false

        let config = URLSessionConfiguration.default
        session = URLSession(configuration: config)

        // Build TXT record for Bonjour advertisement
        var txt = NWTXTRecord()
        txt["device_id"] = deviceId
        txt["device_name"] = deviceName
        txt["platform"] = "macos"
        txt["mesh_id"] = meshId
        txt["api_version"] = "2"

        do {
            try server.start(advertiseTXT: txt)
        } catch {
            logger.error("Failed to start PeerServer: \(error.localizedDescription)")
            return
        }

        startBrowsing()

        isRunning = true
        logger.info("PeerManager started (mesh=\(self.meshId.prefix(8))...)")
    }

    func stop() {
        stopped = true
        isRunning = false

        // Cancel reconnect tasks
        for (_, task) in reconnectTasks {
            task.cancel()
        }
        reconnectTasks.removeAll()

        // Cancel outbound receive tasks
        for (_, task) in outboundReceiveTasks {
            task.cancel()
        }
        outboundReceiveTasks.removeAll()

        // Close outbound connections
        for (_, ws) in outbound {
            ws.cancel(with: .goingAway, reason: nil)
        }
        outbound.removeAll()

        // Stop mDNS browser
        browser?.cancel()
        browser = nil

        // Stop server
        server.stop()

        session?.invalidateAndCancel()
        session = nil

        connectedPeers.removeAll()
        logger.info("PeerManager stopped")
    }

    // MARK: - Broadcast

    /// Broadcast a change to all peers (inbound via server + outbound).
    func broadcastChange(_ change: [String: Any]) {
        var msg = change
        var hopPath = msg["hop_path"] as? [String] ?? []
        if !hopPath.contains(deviceId) {
            hopPath.append(deviceId)
        }
        msg["hop_path"] = hopPath

        guard let data = try? JSONSerialization.data(withJSONObject: msg) else { return }

        // Broadcast to inbound connections via server
        server.broadcastChange(data, hopPath: hopPath)

        // Send to all outbound connections
        guard let text = String(data: data, encoding: .utf8) else { return }
        for (peerDeviceId, ws) in outbound {
            if hopPath.contains(peerDeviceId) { continue }
            Task {
                do {
                    try await ws.send(.string(text))
                } catch {
                    logger.error("Failed to send to outbound \(peerDeviceId): \(error.localizedDescription)")
                }
            }
        }
    }

    // MARK: - mDNS Browsing

    private func startBrowsing() {
        let params = NWParameters()
        params.includePeerToPeer = true

        let browser = NWBrowser(for: .bonjour(type: "_ark-peer._tcp.", domain: nil), using: params)
        self.browser = browser

        browser.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                switch state {
                case .ready:
                    self?.logger.info("Browsing for _ark-peer._tcp. peers")
                case .failed(let error):
                    self?.logger.error("Browser failed: \(error.localizedDescription)")
                default:
                    break
                }
            }
        }

        browser.browseResultsChangedHandler = { [weak self] results, changes in
            Task { @MainActor in
                guard let self else { return }
                for change in changes {
                    switch change {
                    case .added(let result):
                        self.handleDiscoveredService(result)
                    case .removed(let result):
                        self.handleLostService(result)
                    default:
                        break
                    }
                }
            }
        }

        browser.start(queue: .main)
    }

    private func handleDiscoveredService(_ result: NWBrowser.Result) {
        guard case .service(let name, _, _, _) = result.endpoint else { return }

        // Extract TXT record
        if case .bonjour(let txtRecord) = result.metadata {
            let peerMeshId = txtRecord["mesh_id"] ?? ""
            let peerDeviceId = txtRecord["device_id"] ?? name

            // Only connect to peers in the same mesh
            guard peerMeshId == meshId else {
                logger.info("Ignoring peer \(name) with different mesh_id")
                return
            }

            // Skip self
            guard peerDeviceId != deviceId else { return }

            // Skip if already connected (inbound or outbound)
            guard !outbound.keys.contains(peerDeviceId) else { return }
            guard !server.connectedPeerIds.contains(peerDeviceId) else { return }

            let peerDeviceName = txtRecord["device_name"] ?? name
            let peerPlatform = txtRecord["platform"] ?? "unknown"

            logger.info("Discovered peer: \(peerDeviceName) (\(peerDeviceId))")

            // Resolve and connect
            resolveAndConnect(result, peerDeviceId: peerDeviceId, peerDeviceName: peerDeviceName, peerPlatform: peerPlatform)
        }
    }

    private func handleLostService(_ result: NWBrowser.Result) {
        guard case .service(let name, _, _, _) = result.endpoint else { return }

        var peerDeviceId = name
        if case .bonjour(let txtRecord) = result.metadata {
            peerDeviceId = txtRecord["device_id"] ?? name
        }

        logger.info("Peer lost: \(peerDeviceId)")

        // Close outbound connection
        if let ws = outbound[peerDeviceId] {
            ws.cancel(with: .goingAway, reason: nil)
            outbound.removeValue(forKey: peerDeviceId)
        }
        outboundReceiveTasks[peerDeviceId]?.cancel()
        outboundReceiveTasks.removeValue(forKey: peerDeviceId)
        reconnectTasks[peerDeviceId]?.cancel()
        reconnectTasks.removeValue(forKey: peerDeviceId)

        updateConnectedPeers()
    }

    // MARK: - Outbound connection

    private func resolveAndConnect(_ result: NWBrowser.Result, peerDeviceId: String, peerDeviceName: String, peerPlatform: String) {
        // Use NWConnection to resolve the endpoint to host:port
        let connection = NWConnection(to: result.endpoint, using: .tcp)
        connection.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                guard let self else { return }
                if case .ready = state {
                    if let path = connection.currentPath,
                       let endpoint = path.remoteEndpoint,
                       case .hostPort(let host, let port) = endpoint {

                        let hostStr: String
                        switch host {
                        case .ipv4(let addr): hostStr = "\(addr)"
                        case .ipv6(let addr): hostStr = "[\(addr)]"
                        case .name(let name, _): hostStr = name
                        @unknown default: hostStr = "localhost"
                        }

                        connection.cancel()
                        self.connectOutbound(
                            host: hostStr,
                            port: port.rawValue,
                            peerDeviceId: peerDeviceId,
                            peerDeviceName: peerDeviceName
                        )
                    }
                }
            }
        }
        connection.start(queue: .main)

        // Timeout resolution
        Task {
            try? await Task.sleep(for: .seconds(5))
            connection.cancel()
        }
    }

    private func connectOutbound(host: String, port: UInt16, peerDeviceId: String, peerDeviceName: String) {
        guard !stopped else { return }
        guard !outbound.keys.contains(peerDeviceId) else { return }
        guard !server.connectedPeerIds.contains(peerDeviceId) else { return }
        guard let session else { return }

        let urlString = "ws://\(host):\(port)"
        guard let url = URL(string: urlString) else {
            logger.error("Invalid outbound URL: \(urlString)")
            return
        }

        logger.info("Connecting to peer \(peerDeviceName) at \(urlString)")

        let ws = session.webSocketTask(with: url)
        ws.resume()

        // Send peer_hello
        let hello = PeerHello.create(
            deviceId: deviceId,
            deviceName: deviceName,
            platform: "macos",
            meshSecret: meshSecret
        )

        Task {
            do {
                let data = try JSONEncoder().encode(hello)
                guard let text = String(data: data, encoding: .utf8) else { return }
                try await ws.send(.string(text))

                // Wait for peer_hello_ack
                let response = try await ws.receive()
                guard case .string(let ackText) = response,
                      let ackData = ackText.data(using: .utf8),
                      let ackJson = try? JSONSerialization.jsonObject(with: ackData) as? [String: Any],
                      let ackType = ackJson["type"] as? String,
                      ackType == "peer_hello_ack"
                else {
                    logger.warning("Invalid ack from \(peerDeviceName), closing")
                    ws.cancel(with: .normalClosure, reason: nil)
                    return
                }

                // Check ok
                guard ackJson["ok"] as? Bool == true else {
                    let error = ackJson["error"] as? String ?? "unknown"
                    logger.warning("Peer \(peerDeviceName) rejected: \(error)")
                    ws.cancel(with: .normalClosure, reason: nil)
                    return
                }

                // Verify ack HMAC
                if let ackNonce = ackJson["nonce"] as? String,
                   let ackHmac = ackJson["auth_hmac"] as? String {
                    guard PeerProtocol.verifyAuthHmac(meshSecret: meshSecret, nonce: ackNonce, provided: ackHmac) else {
                        logger.warning("Ack HMAC verification failed from \(peerDeviceName)")
                        ws.cancel(with: .normalClosure, reason: nil)
                        return
                    }
                }

                // Connected
                outbound[peerDeviceId] = ws
                updateConnectedPeers()
                logger.info("Connected to peer: \(peerDeviceName) (\(peerDeviceId))")

                // Start receive loop
                startOutboundReceiveLoop(ws, peerDeviceId: peerDeviceId, peerDeviceName: peerDeviceName)

            } catch {
                logger.error("Failed to connect to \(peerDeviceName): \(error.localizedDescription)")
                ws.cancel(with: .normalClosure, reason: nil)
                scheduleReconnect(peerDeviceId: peerDeviceId, peerDeviceName: peerDeviceName, host: host, port: port)
            }
        }
    }

    private func startOutboundReceiveLoop(_ ws: URLSessionWebSocketTask, peerDeviceId: String, peerDeviceName: String) {
        outboundReceiveTasks[peerDeviceId]?.cancel()
        outboundReceiveTasks[peerDeviceId] = Task { [weak self] in
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

                    if type == "change" {
                        await MainActor.run {
                            self.handleIncomingChange(json, fromDevice: peerDeviceId)
                        }
                    }
                } catch {
                    if !Task.isCancelled {
                        await MainActor.run { [weak self] in
                            guard let self else { return }
                            self.logger.info("Outbound connection to \(peerDeviceName) lost")
                            self.outbound.removeValue(forKey: peerDeviceId)
                            self.outboundReceiveTasks.removeValue(forKey: peerDeviceId)
                            self.updateConnectedPeers()
                        }
                    }
                    break
                }
            }
        }
    }

    // MARK: - Change handling

    private func handleIncomingChange(_ change: [String: Any], fromDevice: String) {
        var hopPath = change["hop_path"] as? [String] ?? []
        if !hopPath.contains(deviceId) {
            hopPath.append(deviceId)
        }

        // Notify the app
        onPeerChange?(change, fromDevice)

        // Re-broadcast to other outbound peers (server already re-broadcasts to inbound)
        var forwarded = change
        forwarded["hop_path"] = hopPath
        guard let forwardData = try? JSONSerialization.data(withJSONObject: forwarded),
              let forwardText = String(data: forwardData, encoding: .utf8)
        else { return }

        for (peerDeviceId, ws) in outbound {
            if peerDeviceId == fromDevice { continue }
            if hopPath.contains(peerDeviceId) { continue }
            Task {
                try? await ws.send(.string(forwardText))
            }
        }
    }

    // MARK: - Reconnect

    private func scheduleReconnect(peerDeviceId: String, peerDeviceName: String, host: String, port: UInt16) {
        guard !stopped else { return }
        guard reconnectTasks[peerDeviceId] == nil else { return }

        reconnectTasks[peerDeviceId] = Task { [weak self] in
            try? await Task.sleep(for: .seconds(15))
            guard !Task.isCancelled else { return }
            await MainActor.run { [weak self] in
                guard let self else { return }
                self.reconnectTasks.removeValue(forKey: peerDeviceId)
                if !self.stopped {
                    self.connectOutbound(host: host, port: port, peerDeviceId: peerDeviceId, peerDeviceName: peerDeviceName)
                }
            }
        }
    }

    // MARK: - Helpers

    private func updateConnectedPeers() {
        connectedPeers = Array(Set(server.connectedPeerIds + Array(outbound.keys)))
    }
}
