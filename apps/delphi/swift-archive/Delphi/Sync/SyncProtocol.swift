import Foundation

// MARK: - Constants

let LAN_SYNC_PORT: UInt16 = 21531
let SYNC_PROTOCOL_VERSION = 1
let MAX_BATCH_SIZE = 100

// MARK: - Peer Record (Syncthing-style multi-address)

struct PeerRecord: Codable, Equatable {
    var device_id: String
    var device_name: String
    var addresses: [String]
    var last_seen: String
    var last_address: String?

    static func == (lhs: PeerRecord, rhs: PeerRecord) -> Bool {
        lhs.device_id == rhs.device_id
    }
}

// MARK: - Sync Entity

struct SyncEntity: Codable {
    let type: String   // "todo", "project", "area", "tag", "heading"
    let id: String
    let data: [String: AnyCodable]
    let hlc: String
    var deleted: Bool?
}

// MARK: - Version Vector: entity_id -> HLC string

typealias VersionVector = [String: String]

// MARK: - Protocol Messages (all JSON-serializable via Codable)

enum SyncMessageType: String, Codable {
    case hello
    case version_vector
    case sync_changes
    case sync_ack
    case live_change
    case live_ack
    case peer_list
    case ping
    case pong
}

// We use JSONSerialization for flexible message handling rather than
// strict Codable, matching the existing LanSyncClient pattern.

// MARK: - HLC Comparison

/// Compare two HLC strings. Returns negative if a < b, 0 if equal, positive if a > b.
func compareHlcStrings(_ a: String, _ b: String) -> Int {
    guard let hlcA = HLC.from(a), let hlcB = HLC.from(b) else {
        return a.compare(b) == .orderedAscending ? -1 : (a == b ? 0 : 1)
    }
    if hlcA < hlcB { return -1 }
    if hlcA == hlcB { return 0 }
    return 1
}

/// Returns true if HLC `a` is strictly newer (greater) than HLC `b`.
func isNewerHlc(_ a: String, _ b: String) -> Bool {
    compareHlcStrings(a, b) > 0
}

// MARK: - Peer Record Merging

/// Merge incoming peer records into existing ones.
/// - Union merge of addresses (no duplicates)
/// - Preserve the newest last_seen
/// - New peers are appended
func mergePeerRecords(_ existing: [PeerRecord], _ incoming: [PeerRecord]) -> [PeerRecord] {
    var map: [String: PeerRecord] = [:]
    for peer in existing {
        map[peer.device_id] = peer
    }
    for inc in incoming {
        if var current = map[inc.device_id] {
            // Union merge addresses
            let addrSet = Set(current.addresses + inc.addresses)
            current.addresses = Array(addrSet)
            // Keep newest last_seen
            if inc.last_seen > current.last_seen {
                current.last_seen = inc.last_seen
                current.device_name = inc.device_name
                if let lastAddr = inc.last_address {
                    current.last_address = lastAddr
                }
            }
            map[inc.device_id] = current
        } else {
            map[inc.device_id] = inc
        }
    }
    return Array(map.values)
}

// MARK: - Own Addresses (getifaddrs)

/// Collect all non-loopback network addresses for this device.
/// Returns strings like "192.168.1.70:21531" or "[fe80::1%en0]:21531".
func getOwnAddresses(port: UInt16 = LAN_SYNC_PORT) -> [String] {
    var addresses: [String] = []
    var ifaddr: UnsafeMutablePointer<ifaddrs>?
    guard getifaddrs(&ifaddr) == 0, let first = ifaddr else { return addresses }
    defer { freeifaddrs(first) }

    var ptr: UnsafeMutablePointer<ifaddrs>? = first
    while let ifa = ptr {
        defer { ptr = ifa.pointee.ifa_next }

        guard let sa = ifa.pointee.ifa_addr else { continue }
        let flags = Int32(ifa.pointee.ifa_flags)
        // Skip loopback
        if flags & IFF_LOOPBACK != 0 { continue }
        // Must be up
        if flags & IFF_UP == 0 { continue }

        let family = sa.pointee.sa_family
        if family == UInt8(AF_INET) {
            var addr = sa.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { $0.pointee }
            var buf = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
            inet_ntop(AF_INET, &addr.sin_addr, &buf, socklen_t(INET_ADDRSTRLEN))
            let ip = String(cString: buf)
            addresses.append("\(ip):\(port)")
        } else if family == UInt8(AF_INET6) {
            var addr = sa.withMemoryRebound(to: sockaddr_in6.self, capacity: 1) { $0.pointee }
            var buf = [CChar](repeating: 0, count: Int(INET6_ADDRSTRLEN))
            inet_ntop(AF_INET6, &addr.sin6_addr, &buf, socklen_t(INET6_ADDRSTRLEN))
            let ip = String(cString: buf)
            let ifName = String(cString: ifa.pointee.ifa_name)
            addresses.append("[\(ip)%\(ifName)]:\(port)")
        }
    }
    return addresses
}

// MARK: - Generate unique ID

func generateSyncId() -> String {
    "\(Int(Date().timeIntervalSince1970 * 1000))-\(String(Int.random(in: 0..<1_000_000)))"
}

// MARK: - Generate HLC string

func generateHlcString(deviceId: String) -> String {
    HLC.now(deviceId: deviceId).description
}
