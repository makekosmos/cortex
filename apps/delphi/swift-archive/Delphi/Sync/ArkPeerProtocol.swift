import Foundation
import CryptoKit

// MARK: - Crypto helpers

enum PeerProtocol {

    /// Derive mesh_id from shared secret: HMAC-SHA256("mesh-id", meshSecret), first 16 hex chars.
    /// Must match the TS implementation: createHmac('sha256', 'mesh-id').update(meshSecret).
    static func computeMeshId(meshSecret: String) -> String {
        let key = SymmetricKey(data: Data("mesh-id".utf8))
        let tag = HMAC<SHA256>.authenticationCode(for: Data(meshSecret.utf8), using: key)
        let hex = tag.map { String(format: "%02x", $0) }.joined()
        return String(hex.prefix(16))
    }

    /// HMAC-SHA256(meshSecret, nonce) for mutual authentication.
    static func computeAuthHmac(meshSecret: String, nonce: String) -> String {
        let key = SymmetricKey(data: Data(meshSecret.utf8))
        let tag = HMAC<SHA256>.authenticationCode(for: Data(nonce.utf8), using: key)
        return tag.map { String(format: "%02x", $0) }.joined()
    }

    /// Constant-time HMAC verification.
    static func verifyAuthHmac(meshSecret: String, nonce: String, provided: String) -> Bool {
        let expected = computeAuthHmac(meshSecret: meshSecret, nonce: nonce)
        // Convert both to Data for constant-time comparison via CryptoKit
        guard let expectedData = Data(hexString: expected),
              let providedData = Data(hexString: provided)
        else { return false }
        return expectedData == providedData
    }

    /// Generate a random 32-byte hex nonce (64 hex characters).
    static func generateNonce() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}

// MARK: - Hex Data helper

private extension Data {
    init?(hexString: String) {
        let len = hexString.count
        guard len % 2 == 0 else { return nil }

        var data = Data(capacity: len / 2)
        var index = hexString.startIndex
        for _ in 0..<len / 2 {
            let nextIndex = hexString.index(index, offsetBy: 2)
            guard let byte = UInt8(hexString[index..<nextIndex], radix: 16) else { return nil }
            data.append(byte)
            index = nextIndex
        }
        self = data
    }
}

// MARK: - Message types

struct PeerHello: Codable {
    let type: String  // "peer_hello"
    let protocolVersion: Int  // 2
    let deviceId: String
    let deviceName: String
    let platform: String
    let meshId: String
    let nonce: String
    let authHmac: String

    enum CodingKeys: String, CodingKey {
        case type
        case protocolVersion = "protocol_version"
        case deviceId = "device_id"
        case deviceName = "device_name"
        case platform
        case meshId = "mesh_id"
        case nonce
        case authHmac = "auth_hmac"
    }

    static func create(
        deviceId: String,
        deviceName: String,
        platform: String,
        meshSecret: String
    ) -> PeerHello {
        let nonce = PeerProtocol.generateNonce()
        return PeerHello(
            type: "peer_hello",
            protocolVersion: 2,
            deviceId: deviceId,
            deviceName: deviceName,
            platform: platform,
            meshId: PeerProtocol.computeMeshId(meshSecret: meshSecret),
            nonce: nonce,
            authHmac: PeerProtocol.computeAuthHmac(meshSecret: meshSecret, nonce: nonce)
        )
    }
}

struct PeerHelloAck: Codable {
    let type: String  // "peer_hello_ack"
    let ok: Bool
    let deviceId: String
    let deviceName: String
    let platform: String
    let nonce: String
    let authHmac: String
    let error: String?

    enum CodingKeys: String, CodingKey {
        case type
        case ok
        case deviceId = "device_id"
        case deviceName = "device_name"
        case platform
        case nonce
        case authHmac = "auth_hmac"
        case error
    }

    static func create(
        ok: Bool,
        deviceId: String,
        deviceName: String,
        platform: String,
        meshSecret: String,
        error: String? = nil
    ) -> PeerHelloAck {
        let nonce = PeerProtocol.generateNonce()
        return PeerHelloAck(
            type: "peer_hello_ack",
            ok: ok,
            deviceId: deviceId,
            deviceName: deviceName,
            platform: platform,
            nonce: nonce,
            authHmac: PeerProtocol.computeAuthHmac(meshSecret: meshSecret, nonce: nonce),
            error: error
        )
    }
}

struct PeerChange: Codable {
    let type: String  // "change"
    let eventId: String
    let changeType: String  // create, update, delete
    let data: [String: AnyCodable]
    let originDevice: String
    let originSeq: Int
    let hlc: String
    var hopPath: [String]

    enum CodingKeys: String, CodingKey {
        case type
        case eventId = "event_id"
        case changeType = "change_type"
        case data
        case originDevice = "origin_device"
        case originSeq = "origin_seq"
        case hlc
        case hopPath = "hop_path"
    }
}

// MARK: - AnyCodable wrapper for heterogeneous JSON

/// A type-erased Codable wrapper so we can encode/decode arbitrary JSON values.
struct AnyCodable: Codable {
    nonisolated(unsafe) let value: Any

    init(_ value: Any) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map(\.value)
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues(\.value)
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Unsupported type")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            try container.encodeNil()
        }
    }
}
