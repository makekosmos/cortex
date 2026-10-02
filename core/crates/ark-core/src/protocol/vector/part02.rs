#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compute_vector_diff_missing() {
        let local = VersionVector::new();
        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.contains("e1"));
    }

    #[test]
    fn compute_vector_diff_outdated() {
        let mut local = VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-02T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.contains("e1"));
    }

    #[test]
    fn compute_vector_diff_up_to_date() {
        let mut local = VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-02T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let needed = compute_vector_diff(&local, &remote);
        assert!(needed.is_empty());
    }

    #[test]
    fn compute_local_excess_finds_local_only_entities() {
        let mut local = VersionVector::new();
        local.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );
        local.insert(
            "e2".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let mut remote = VersionVector::new();
        remote.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let excess = compute_local_excess(&local, &remote);
        assert!(excess.contains("e2"));
        assert!(!excess.contains("e1"));
    }

    #[test]
    fn split_into_batches_by_count() {
        let entities: Vec<SyncEntity> = (0..250)
            .map(|i| SyncEntity {
                entity_type: "todo".to_string(),
                id: format!("t{i}"),
                data: serde_json::Map::new(),
                hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
                deleted: None,
                origin_device_id: None,
                origin_seq: None,
            })
            .collect();

        let batches = split_into_batches(&entities);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 100);
        assert_eq!(batches[1].len(), 100);
        assert_eq!(batches[2].len(), 50);
    }

    #[test]
    fn split_into_batches_by_bytes() {
        let entities: Vec<SyncEntity> = (0..10)
            .map(|i| {
                let mut data = serde_json::Map::new();
                data.insert("big".to_string(), json!("x".repeat(200_000)));
                SyncEntity {
                    entity_type: "todo".to_string(),
                    id: format!("t{i}"),
                    data,
                    hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
                    deleted: None,
                    origin_device_id: None,
                    origin_seq: None,
                }
            })
            .collect();

        let batches = split_into_batches(&entities);
        assert!(batches.len() >= 2);
        for batch in &batches {
            let total: usize = batch
                .iter()
                .map(|entity| serde_json::to_string(entity).unwrap().len())
                .sum();
            if batch.len() > 1 {
                assert!(total <= MAX_BATCH_BYTES + 300_000);
            }
        }
    }

    #[test]
    fn split_into_batches_empty() {
        let batches = split_into_batches(&[]);
        assert!(batches.is_empty());
    }

    #[test]
    fn merge_peer_records_adds_new_peer() {
        let existing: Vec<PeerRecord> = vec![];
        let incoming = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone".to_string(),
            addresses: vec!["10.0.0.1:21531".to_string()],
            last_seen: "2026-01-01T00:00:00.000Z".to_string(),
            last_address: None,
        }];
        let merged = merge_peer_records(&existing, &incoming);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].device_id, "d1");
    }

    #[test]
    fn merge_peer_records_unions_addresses_and_keeps_newest_metadata() {
        let existing = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone".to_string(),
            addresses: vec!["10.0.0.1:21531".to_string()],
            last_seen: "2026-01-01T00:00:00.000Z".to_string(),
            last_address: None,
        }];
        let incoming = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone v2".to_string(),
            addresses: vec![
                "10.0.0.1:21531".to_string(),
                "192.168.1.50:21531".to_string(),
            ],
            last_seen: "2026-01-02T00:00:00.000Z".to_string(),
            last_address: Some("192.168.1.50:21531".to_string()),
        }];

        let merged = merge_peer_records(&existing, &incoming);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].addresses.contains(&"10.0.0.1:21531".to_string()));
        assert!(merged[0]
            .addresses
            .contains(&"192.168.1.50:21531".to_string()));
        assert_eq!(merged[0].device_name, "Phone v2");
        assert_eq!(merged[0].last_seen, "2026-01-02T00:00:00.000Z");
        assert_eq!(
            merged[0].last_address,
            Some("192.168.1.50:21531".to_string())
        );
    }

    #[test]
    fn merge_peer_records_keeps_older_last_address() {
        let existing = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone".to_string(),
            addresses: vec!["10.0.0.1:21531".to_string()],
            last_seen: "2026-01-02T00:00:00.000Z".to_string(),
            last_address: Some("10.0.0.1:21531".to_string()),
        }];
        let incoming = vec![PeerRecord {
            device_id: "d1".to_string(),
            device_name: "Phone old".to_string(),
            addresses: vec!["10.0.0.2:21531".to_string()],
            last_seen: "2026-01-01T00:00:00.000Z".to_string(),
            last_address: Some("10.0.0.2:21531".to_string()),
        }];

        let merged = merge_peer_records(&existing, &incoming);
        assert_eq!(merged[0].device_name, "Phone");
        assert_eq!(merged[0].last_address, Some("10.0.0.1:21531".to_string()));
    }
}
