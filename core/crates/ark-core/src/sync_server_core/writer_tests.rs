    use std::sync::atomic::{AtomicBool, Ordering};

    struct RevocableBackend {
        allowed: AtomicBool,
    }

    #[async_trait::async_trait]
    impl StorageBackend for RevocableBackend {
        async fn validate_outbound_signed_integration_frame(
            &self,
            _frame: &SignedSyncEnvelope,
            _expected_space_id: &str,
            _expected_origin_node_id: &str,
        ) -> Result<(), String> {
            self.allowed
                .load(Ordering::SeqCst)
                .then_some(())
                .ok_or_else(|| "revoked".to_string())
        }

        async fn load_entities(&self, _vector: &VersionVector) -> Vec<SyncEntity> {
            Vec::new()
        }

        async fn load_entities_page(
            &self,
            _vector: &VersionVector,
            _offset: usize,
            _limit: usize,
        ) -> Vec<SyncEntity> {
            Vec::new()
        }

        async fn apply_entity(&self, _entity: &SyncEntity) -> Result<(), String> {
            Ok(())
        }

        async fn get_kv(&self, _key: &str) -> Option<String> {
            None
        }

        async fn set_kv(&self, _key: &str, _value: &str) {}
    }

    #[tokio::test]
    async fn queued_frame_is_dropped_when_revoked_before_writer_recheck() {
        let backend_impl = Arc::new(RevocableBackend {
            allowed: AtomicBool::new(true),
        });
        let backend = backend_impl.clone() as Arc<dyn StorageBackend>;
        let frame =
            SignedSyncEnvelope::new("space", "origin", "recipient", 1, "message", vec![], "sig");
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(frame).unwrap();
        backend_impl.allowed.store(false, Ordering::SeqCst);

        let queued = rx.recv().await.expect("queued frame");
        assert!(
            !signed_integration_write_is_authorized(&backend, &queued, "space", "origin",).await,
            "writer must drop a frame revoked after enqueue"
        );
    }
