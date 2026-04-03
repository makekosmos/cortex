import Foundation

/// Hybrid Logical Clock for P2P sync conflict resolution.
///
/// Format: "<ISO8601>:<counter:06d>:<device_id>"
/// Example: "2026-03-28T14:30:00.123Z:000042:mac-a1b2c3d4"
///
/// Comparison is lexicographic: timestamp first, then counter, then device_id as tiebreaker.
struct HLC: Comparable, CustomStringConvertible, Sendable {
    let wallTime: String  // ISO 8601 UTC
    let counter: Int
    let deviceId: String

    nonisolated(unsafe) private static let isoFormatter: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    private static func nowWall() -> String {
        isoFormatter.string(from: Date())
    }

    /// Create a new HLC stamped with the current wall-clock time.
    static func now(deviceId: String) -> HLC {
        HLC(wallTime: nowWall(), counter: 0, deviceId: deviceId)
    }

    /// Advance the clock for a local event.
    func tick() -> HLC {
        let now = Self.nowWall()
        if now > wallTime {
            return HLC(wallTime: now, counter: 0, deviceId: deviceId)
        }
        return HLC(wallTime: wallTime, counter: counter + 1, deviceId: deviceId)
    }

    /// Merge with a remote HLC (on receiving a change from another peer).
    func merge(remote: HLC) -> HLC {
        let now = Self.nowWall()
        let maxWall = max(wallTime, max(remote.wallTime, now))

        let newCounter: Int
        if maxWall == wallTime && maxWall == remote.wallTime {
            newCounter = max(counter, remote.counter) + 1
        } else if maxWall == wallTime {
            newCounter = counter + 1
        } else if maxWall == remote.wallTime {
            newCounter = remote.counter + 1
        } else {
            // now is strictly the largest
            newCounter = 0
        }

        return HLC(wallTime: maxWall, counter: newCounter, deviceId: deviceId)
    }

    // MARK: - CustomStringConvertible

    var description: String {
        let paddedCounter = String(format: "%06d", counter)
        return "\(wallTime):\(paddedCounter):\(deviceId)"
    }

    // MARK: - Parsing

    /// Parse from string format "wallTime:counter:deviceId".
    /// The wallTime itself contains colons (ISO 8601), so we split from the right.
    static func from(_ string: String) -> HLC? {
        // Find the last two colons to split counter and deviceId
        guard let lastColon = string.lastIndex(of: ":") else { return nil }
        let beforeLast = string[string.startIndex..<lastColon]
        guard let secondLastColon = beforeLast.lastIndex(of: ":") else { return nil }

        let wallTime = String(string[string.startIndex..<secondLastColon])
        let counterStr = String(string[string.index(after: secondLastColon)..<lastColon])
        let deviceId = String(string[string.index(after: lastColon)...])

        guard let counter = Int(counterStr) else { return nil }
        return HLC(wallTime: wallTime, counter: counter, deviceId: deviceId)
    }

    // MARK: - Comparable

    static func < (lhs: HLC, rhs: HLC) -> Bool {
        if lhs.wallTime != rhs.wallTime { return lhs.wallTime < rhs.wallTime }
        if lhs.counter != rhs.counter { return lhs.counter < rhs.counter }
        return lhs.deviceId < rhs.deviceId
    }

    static func == (lhs: HLC, rhs: HLC) -> Bool {
        lhs.wallTime == rhs.wallTime && lhs.counter == rhs.counter && lhs.deviceId == rhs.deviceId
    }
}
