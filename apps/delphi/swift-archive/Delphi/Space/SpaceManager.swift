import Foundation
import CryptoKit
import Security

/// Manages the active Ark Space — the shared secret identifying the P2P mesh.
///
/// A space code is a 12-character Base32-Crockford string (no I, L, O, U),
/// formatted as XXXX-XXXX-XXXX. The mesh secret = normalized code.
/// The space ID = SHA-256(normalized code)[:16 hex chars].
@Observable
final class SpaceManager {

    // MARK: - UserDefaults keys

    private enum Keys {
        static let activeSpaceCode = "ark.space.activeCode"
        static let savedSpaceCodes = "ark.space.savedCodes"
    }

    private let defaults = UserDefaults.standard

    // MARK: - Base32-Crockford alphabet (no I, L, O, U)

    private static let alphabet: [Character] = Array("0123456789ABCDEFGHJKMNPQRSTVWXYZ")

    // MARK: - Code generation

    /// Generate a random 12-character Base32-Crockford code formatted as "XXXX-XXXX-XXXX".
    func generateSpaceCode() -> String {
        var bytes = [UInt8](repeating: 0, count: 8)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)

        // Pack 8 bytes into a UInt64, extract twelve 5-bit groups
        var n: UInt64 = 0
        for byte in bytes {
            n = (n << 8) | UInt64(byte)
        }

        var chars = [Character]()
        for i in stride(from: 55, through: 0, by: -5) {
            if chars.count == 12 { break }
            let idx = Int((n >> i) & 0x1F)
            chars.append(SpaceManager.alphabet[idx])
        }

        let s = String(chars.prefix(12))
        let a = s.prefix(4)
        let b = s.dropFirst(4).prefix(4)
        let c = s.dropFirst(8).prefix(4)
        return "\(a)-\(b)-\(c)"
    }

    /// Normalize a code: remove dashes and uppercase.
    func normalizeCode(_ code: String) -> String {
        code.replacingOccurrences(of: "-", with: "").uppercased()
    }

    /// Format a raw 12-char code as "XXXX-XXXX-XXXX".
    func formatCode(_ code: String) -> String {
        let raw = normalizeCode(code)
        guard raw.count == 12 else { return code }
        let a = raw.prefix(4)
        let b = raw.dropFirst(4).prefix(4)
        let c = raw.dropFirst(8).prefix(4)
        return "\(a)-\(b)-\(c)"
    }

    /// Derive a stable space ID: SHA-256(normalized code) → first 16 hex chars.
    func deriveSpaceId(from code: String) -> String {
        let normalized = normalizeCode(code)
        let data = Data(normalized.utf8)
        let digest = SHA256.hash(data: data)
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        return String(hex.prefix(16))
    }

    // MARK: - Persistence

    var activeSpaceCode: String? {
        get { defaults.string(forKey: Keys.activeSpaceCode) }
        set {
            if let v = newValue {
                defaults.set(v, forKey: Keys.activeSpaceCode)
            } else {
                defaults.removeObject(forKey: Keys.activeSpaceCode)
            }
        }
    }

    var savedSpaceCodes: [String] {
        get { defaults.stringArray(forKey: Keys.savedSpaceCodes) ?? [] }
        set { defaults.set(newValue, forKey: Keys.savedSpaceCodes) }
    }

    var isSpaceConfigured: Bool {
        !(activeSpaceCode ?? "").isEmpty
    }

    func addSavedSpaceCode(_ code: String) {
        let normalized = normalizeCode(code)
        var codes = savedSpaceCodes
        if !codes.map({ normalizeCode($0) }).contains(normalized) {
            codes.append(code)
            savedSpaceCodes = codes
        }
    }

    func clearActiveSpace() {
        activeSpaceCode = nil
    }

    /// Validate and normalize user input for a space code.
    /// Strips dashes/spaces, uppercases, validates Base32-Crockford alphabet.
    /// Returns raw 12-char code or nil if invalid.
    func parseCode(_ input: String) -> String? {
        let clean = input.replacingOccurrences(of: "-", with: "")
            .replacingOccurrences(of: " ", with: "")
            .uppercased()
        guard clean.count == 12 else { return nil }
        let validChars = Set(SpaceManager.alphabet)
        guard clean.allSatisfy({ validChars.contains($0) }) else { return nil }
        return clean
    }

    // MARK: - QR Payload

    /// Generate a QR payload: `ark://join?code=XXXX-XXXX-XXXX&addrs=ip1:port,ip2:port`
    func generateQrPayload(code: String, addresses: [String]) -> String {
        let formatted = formatCode(code)
        let addrs = addresses.joined(separator: ",")
        return "ark://join?code=\(formatted)&addrs=\(addrs)"
    }

    /// Parse a QR payload back into (code, addresses).
    /// Returns nil if the payload is not valid.
    func parseQrPayload(_ payload: String) -> (code: String, addresses: [String])? {
        let trimmed = payload.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("ark://join?") else { return nil }

        let queryPart = String(trimmed.dropFirst("ark://join?".count))
        var params: [String: String] = [:]
        for pair in queryPart.split(separator: "&") {
            let kv = pair.split(separator: "=", maxSplits: 1)
            if kv.count == 2 {
                params[String(kv[0])] = String(kv[1])
            }
        }

        guard let codeParam = params["code"] else { return nil }
        let normalized = normalizeCode(codeParam)
        guard normalized.count == 12 else { return nil }

        let addrsStr = params["addrs"] ?? ""
        let addresses = addrsStr.split(separator: ",").map { String($0) }.filter { !$0.isEmpty }

        return (code: normalized, addresses: addresses)
    }
}
