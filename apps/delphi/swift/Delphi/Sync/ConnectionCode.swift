import Foundation

/// Encodes/decodes IPv4 addresses to/from 7-character Base32-Crockford connection codes.
///
/// Format: `XXXX-XXX` (7 chars, dash after 4th).
/// Encoding: 4 bytes (32 bits) -> 7 x 5-bit Base32 chars (35 bits, top 3 always 0).
enum ConnectionCode {

    // Base32-Crockford alphabet (no I, L, O, U)
    private static let alphabet: [Character] = Array("0123456789ABCDEFGHJKMNPQRSTVWXYZ")

    // Reverse lookup: character -> 5-bit value
    private static let reverseMap: [Character: Int] = {
        var map: [Character: Int] = [:]
        for (i, c) in alphabet.enumerated() {
            map[c] = i
        }
        return map
    }()

    /// Decode a 7-character Base32-Crockford code to an IPv4 address string.
    /// Returns nil if the code is invalid.
    static func decodeCodeToIp(_ code: String) -> String? {
        guard let clean = parseCode(code) else { return nil }

        var num: UInt64 = 0
        for ch in clean {
            guard let val = reverseMap[ch] else { return nil }
            num = num * 32 + UInt64(val)
        }

        // 7 x 5 = 35 bits; top 3 must be 0 for a valid 32-bit IP
        guard num <= 0xFFFF_FFFF else { return nil }

        let a = (num >> 24) & 0xFF
        let b = (num >> 16) & 0xFF
        let c = (num >> 8) & 0xFF
        let d = num & 0xFF

        return "\(a).\(b).\(c).\(d)"
    }

    /// Format a raw 7-char code as `XXXX-XXX`.
    static func formatCode(_ code: String) -> String {
        let clean = code.replacingOccurrences(of: "-", with: "")
            .replacingOccurrences(of: " ", with: "")
            .uppercased()
        guard clean.count == 7 else { return code }
        let a = clean.prefix(4)
        let b = clean.dropFirst(4)
        return "\(a)-\(b)"
    }

    /// Validate and normalize user input.
    /// Strips dashes/spaces, uppercases, validates Base32-Crockford alphabet.
    /// Returns raw 7-char code or nil if invalid.
    static func parseCode(_ input: String) -> String? {
        let clean = input.replacingOccurrences(of: "-", with: "")
            .replacingOccurrences(of: " ", with: "")
            .uppercased()
        guard clean.count == 7 else { return nil }
        guard clean.allSatisfy({ reverseMap[$0] != nil }) else { return nil }
        return clean
    }
}
