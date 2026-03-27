import Foundation
import Network
import os

/// Discovers Ark servers on the local network via Bonjour/mDNS.
///
/// Ark advertises itself as `_ark-sync._tcp.` on the LAN.
/// When found, we resolve the hostname:port and build the server URL.
@Observable
@MainActor
final class ArkDiscovery {
    var discoveredUrl: String?
    var isSearching = false

    private var browser: NWBrowser?
    private var searchTask: Task<Void, Never>?
    private let logger = Logger(subsystem: "com.kosmos.delphi", category: "ArkDiscovery")

    /// Start searching for Ark servers on the local network.
    /// Automatically stops after `timeout` seconds.
    func startSearching(timeout: TimeInterval = 5) {
        guard !isSearching else { return }
        isSearching = true
        discoveredUrl = nil

        let parameters = NWParameters()
        parameters.includePeerToPeer = true

        let browser = NWBrowser(for: .bonjour(type: "_ark-sync._tcp.", domain: nil), using: parameters)
        self.browser = browser

        browser.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                guard let self else { return }
                switch state {
                case .failed(let error):
                    self.logger.error("Browser failed: \(error.localizedDescription)")
                    self.stopSearching()
                case .cancelled:
                    self.isSearching = false
                default:
                    break
                }
            }
        }

        browser.browseResultsChangedHandler = { [weak self] results, _ in
            Task { @MainActor in
                guard let self else { return }
                for result in results {
                    if case .service(let name, let type, let domain, _) = result.endpoint {
                        self.logger.info("Found Ark service: \(name) (\(type) in \(domain))")
                        self.resolveService(result)
                        return
                    }
                }
            }
        }

        browser.start(queue: .main)

        // Auto-stop after timeout
        searchTask = Task {
            try? await Task.sleep(for: .seconds(timeout))
            if !Task.isCancelled {
                stopSearching()
            }
        }
    }

    func stopSearching() {
        searchTask?.cancel()
        searchTask = nil
        browser?.cancel()
        browser = nil
        isSearching = false
    }

    /// Also check localhost directly as a fast fallback (common case: Ark runs on same machine).
    func checkLocalhost() async -> String? {
        let candidates = [
            "http://localhost:8000",
            "http://127.0.0.1:8000",
        ]

        for candidate in candidates {
            guard let url = URL(string: "\(candidate)/health") else { continue }
            var request = URLRequest(url: url)
            request.timeoutInterval = 2

            if let (_, response) = try? await URLSession.shared.data(for: request),
               let http = response as? HTTPURLResponse,
               http.statusCode == 200 {
                logger.info("Ark found at \(candidate)")
                return candidate
            }
        }
        return nil
    }

    // MARK: - Private

    private func resolveService(_ result: NWBrowser.Result) {
        let connection = NWConnection(to: result.endpoint, using: .tcp)
        connection.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                guard let self else { return }
                if case .ready = state {
                    if let path = connection.currentPath,
                       let endpoint = path.remoteEndpoint {
                        switch endpoint {
                        case .hostPort(let host, let port):
                            let hostStr: String
                            switch host {
                            case .ipv4(let addr):
                                hostStr = "\(addr)"
                            case .ipv6(let addr):
                                hostStr = "[\(addr)]"
                            case .name(let name, _):
                                hostStr = name
                            @unknown default:
                                hostStr = "localhost"
                            }
                            self.discoveredUrl = "http://\(hostStr):\(port)"
                            self.logger.info("Resolved Ark at \(self.discoveredUrl ?? "")")
                        default:
                            break
                        }
                    }
                    connection.cancel()
                    self.stopSearching()
                }
            }
        }
        connection.start(queue: .main)

        // Timeout the resolution
        Task {
            try? await Task.sleep(for: .seconds(3))
            connection.cancel()
        }
    }
}
