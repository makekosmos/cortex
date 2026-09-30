#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]

use engine::package_worker_broker::SnapshotRegistry;

// Runs in its own test binary so /proc/self/fd reflects only this process;
// inside the unit-test binary parallel tests hold transient descriptors and
// make a process-wide fd count nondeterministic.
#[test]
fn fd_counter_stays_bounded_across_ten_thousand_real_snapshot_cycles() {
    fn fd_count() -> usize {
        std::fs::read_dir("/proc/self/fd").unwrap().count()
    }
    let before = fd_count();
    let snapshots = SnapshotRegistry::new();
    let bytes: Vec<u8> = (0..=255).cycle().take(65_537).collect();
    for _ in 0..10_000 {
        let handle = snapshots
            .reserve("owner", "package", "bundled", bytes.clone())
            .unwrap();
        let mut received = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            let chunk = snapshots
                .chunk(&handle, "owner", offset, 16 * 1024)
                .unwrap();
            assert!(!chunk.is_empty());
            received.extend_from_slice(&chunk);
            offset += chunk.len();
        }
        assert_eq!(received, bytes);
        snapshots.close(&handle, "owner").unwrap();
        assert_eq!(snapshots.len(), 0);
    }
    assert!(fd_count().saturating_sub(before) <= 2);
}
