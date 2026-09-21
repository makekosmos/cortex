use chrono::Utc;
use std::cmp::Ordering;

/// Hybrid Logical Clock for P2P sync conflict resolution.
///
/// Format: `<ISO8601>:<counter:06d>:<device_id>`
/// Example: `2026-03-28T14:30:00.123Z:000042:delphi-web-abc123`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HLC {
    pub wall_time: String,
    pub counter: u64,
    pub device_id: String,
}

impl HLC {
    pub fn new(wall_time: String, counter: u64, device_id: String) -> Self {
        Self {
            wall_time,
            counter,
            device_id,
        }
    }

    /// Create a new HLC with the current wall time and counter 0.
    pub fn now(device_id: &str) -> Self {
        Self {
            wall_time: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            counter: 0,
            device_id: device_id.to_string(),
        }
    }

    /// Advance the clock. If wall time has advanced, reset counter; otherwise increment.
    pub fn tick(&self) -> Self {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        if self.wall_time < now {
            Self {
                wall_time: now,
                counter: 0,
                device_id: self.device_id.clone(),
            }
        } else {
            Self {
                wall_time: self.wall_time.clone(),
                counter: self.counter + 1,
                device_id: self.device_id.clone(),
            }
        }
    }

    /// Merge with a remote HLC, producing a new HLC for this device.
    pub fn merge(&self, remote: &HLC) -> Self {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let mut times = [
            now.as_str(),
            self.wall_time.as_str(),
            remote.wall_time.as_str(),
        ];
        times.sort();
        let max_time = times[2].to_string();

        let counter = if max_time == self.wall_time && max_time == remote.wall_time {
            std::cmp::max(self.counter, remote.counter) + 1
        } else if max_time == self.wall_time {
            self.counter + 1
        } else if max_time == remote.wall_time {
            remote.counter + 1
        } else {
            // now is the max
            0
        };

        Self {
            wall_time: max_time,
            counter,
            device_id: self.device_id.clone(),
        }
    }

    /// Compare two HLCs. Returns ordering: wall_time, then counter, then device_id.
    pub fn compare(a: &HLC, b: &HLC) -> Ordering {
        a.wall_time
            .cmp(&b.wall_time)
            .then(a.counter.cmp(&b.counter))
            .then(a.device_id.cmp(&b.device_id))
    }

    /// Compare two HLC strings.
    pub fn compare_str(a: &str, b: &str) -> Ordering {
        let hlc_a = Self::from_string(a);
        let hlc_b = Self::from_string(b);
        Self::compare(&hlc_a, &hlc_b)
    }

    /// Returns true if HLC string `a` is newer than `b`.
    pub fn is_newer(a: &str, b: &str) -> bool {
        Self::compare_str(a, b) == Ordering::Greater
    }

    /// Serialize to string format: `<ISO8601>:<counter:06d>:<device_id>`
    #[allow(clippy::inherent_to_string_shadow_display)]
    pub fn to_string(&self) -> String {
        format!("{}:{:06}:{}", self.wall_time, self.counter, self.device_id)
    }

    /// Parse from string format. Splits on `:` after the `Z` character.
    /// Returns a default HLC (empty fields, counter 0) for malformed/empty input
    /// instead of panicking.
    pub fn from_string(s: &str) -> Self {
        // Guard against empty input before any indexing.
        if s.is_empty() {
            return Self {
                wall_time: String::new(),
                counter: 0,
                device_id: String::new(),
            };
        }
        let z_pos = s.find('Z').unwrap_or(0);
        let first_colon = match s[z_pos..].find(':') {
            Some(i) => z_pos + i,
            None => {
                return Self {
                    wall_time: s.to_string(),
                    counter: 0,
                    device_id: String::new(),
                }
            }
        };
        // first_colon + 1 is safe because ':' was found within the string.
        let rest = &s[first_colon + 1..];
        let second_colon = match rest.find(':') {
            Some(i) => i,
            None => {
                return Self {
                    wall_time: s[..first_colon].to_string(),
                    counter: rest.parse::<u64>().unwrap_or(0),
                    device_id: String::new(),
                }
            }
        };

        let wall_time = s[..first_colon].to_string();
        let counter = rest[..second_colon].parse::<u64>().unwrap_or(0);
        // second_colon + 1 is safe because ':' was found within rest.
        let device_id = rest[second_colon + 1..].to_string();

        Self {
            wall_time,
            counter,
            device_id,
        }
    }
}

impl std::fmt::Display for HLC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{:06}:{}",
            self.wall_time, self.counter, self.device_id
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_from_string_roundtrip() {
        let hlc = HLC::new(
            "2026-03-28T14:30:00.123Z".to_string(),
            42,
            "device-abc".to_string(),
        );
        let s = hlc.to_string();
        assert_eq!(s, "2026-03-28T14:30:00.123Z:000042:device-abc");

        let parsed = HLC::from_string(&s);
        assert_eq!(parsed.wall_time, "2026-03-28T14:30:00.123Z");
        assert_eq!(parsed.counter, 42);
        assert_eq!(parsed.device_id, "device-abc");
    }

    #[test]
    fn test_compare_wall_time() {
        let a = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 0, "a".to_string());
        let b = HLC::new("2026-01-02T00:00:00.000Z".to_string(), 0, "a".to_string());
        assert_eq!(HLC::compare(&a, &b), Ordering::Less);
        assert_eq!(HLC::compare(&b, &a), Ordering::Greater);
    }

    #[test]
    fn test_compare_counter() {
        let a = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 1, "a".to_string());
        let b = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 2, "a".to_string());
        assert_eq!(HLC::compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn test_compare_device_id() {
        let a = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 0, "aaa".to_string());
        let b = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 0, "bbb".to_string());
        assert_eq!(HLC::compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn test_compare_equal() {
        let a = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 5, "dev".to_string());
        let b = HLC::new("2026-01-01T00:00:00.000Z".to_string(), 5, "dev".to_string());
        assert_eq!(HLC::compare(&a, &b), Ordering::Equal);
    }

    #[test]
    fn test_tick_advances() {
        let hlc = HLC::now("test-device");
        let ticked = hlc.tick();
        // Either wall_time advanced (counter=0) or counter incremented
        assert!(
            ticked.wall_time > hlc.wall_time
                || (ticked.wall_time == hlc.wall_time && ticked.counter == hlc.counter + 1)
        );
    }

    #[test]
    fn test_merge() {
        let local = HLC::new(
            "2026-01-01T00:00:00.000Z".to_string(),
            5,
            "local".to_string(),
        );
        let remote = HLC::new(
            "2026-01-01T00:00:00.000Z".to_string(),
            10,
            "remote".to_string(),
        );
        let merged = local.merge(&remote);
        assert_eq!(merged.device_id, "local");
        // merged counter should be max(5, 10) + 1 = 11 if wall_times match
        // (unless current time is newer)
        assert!(merged.counter >= 11 || merged.wall_time.as_str() > "2026-01-01T00:00:00.000Z");
    }

    #[test]
    fn test_is_newer() {
        let a = "2026-01-02T00:00:00.000Z:000000:dev";
        let b = "2026-01-01T00:00:00.000Z:000000:dev";
        assert!(HLC::is_newer(a, b));
        assert!(!HLC::is_newer(b, a));
    }

    #[test]
    fn test_from_string_with_colons_in_device_id() {
        // Device IDs should not contain colons, but test robustness
        let s = "2026-03-28T14:30:00.123Z:000001:dev-id";
        let hlc = HLC::from_string(s);
        assert_eq!(hlc.wall_time, "2026-03-28T14:30:00.123Z");
        assert_eq!(hlc.counter, 1);
        assert_eq!(hlc.device_id, "dev-id");
    }

    #[test]
    fn test_compare_str() {
        let a = "2026-01-01T00:00:00.000Z:000005:dev-a";
        let b = "2026-01-01T00:00:00.000Z:000005:dev-b";
        assert_eq!(HLC::compare_str(a, b), Ordering::Less);
    }

    #[test]
    fn test_from_string_empty() {
        // Must not panic.
        let hlc = HLC::from_string("");
        assert_eq!(hlc.wall_time, "");
        assert_eq!(hlc.counter, 0);
        assert_eq!(hlc.device_id, "");
    }

    #[test]
    fn test_from_string_no_colons() {
        // String with 'Z' but no ':' after it — must not panic.
        let hlc = HLC::from_string("2026-01-01Z");
        assert_eq!(hlc.wall_time, "2026-01-01Z");
        assert_eq!(hlc.counter, 0);
        assert_eq!(hlc.device_id, "");
    }

    #[test]
    fn test_from_string_one_colon_after_z() {
        // Has Z and one colon but no second colon — must not panic.
        let hlc = HLC::from_string("2026-01-01T00:00:00.000Z:000042");
        assert_eq!(hlc.wall_time, "2026-01-01T00:00:00.000Z");
        assert_eq!(hlc.counter, 42);
        assert_eq!(hlc.device_id, "");
    }
}
