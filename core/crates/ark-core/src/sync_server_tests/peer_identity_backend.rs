    #[async_trait::async_trait]
    impl PeerIdentityBackend for MemBackend {
        async fn load_local_sync_profile(
            &self,
            _: &str,
        ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
            Ok(None)
        }

        async fn accept_peer_sync_policy(
            &self,
            _: &str,
            _: i64,
            _: &str,
            _: bool,
            _: &[ark_application::sync_policy::SyncProfileRule],
        ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, String> {
            Err("test_sync_policy_unsupported".into())
        }

        async fn load_accepted_peer_sync_policy(
            &self,
            _: &str,
        ) -> Result<Option<ark_application::sync_policy::AcceptedPeerSyncPolicy>, String> {
            Ok(None)
        }

        async fn sync_session_is_admitted(&self, _: &str) -> Result<bool, String> {
            Err("test_admission_unsupported".into())
        }

        async fn registry_device_is_enabled(&self, _: &str) -> Result<Option<bool>, String> {
            Err("test_registry_unsupported".into())
        }

        async fn authorized_transport_public_key(
            &self,
            _device_id: &str,
        ) -> Result<Option<String>, String> {
            Ok(None)
        }

        async fn load_peer_transport_keys(&self) -> Result<HashMap<String, String>, String> {
            if let Some(error) = self.peer_transport_keys_error.lock().unwrap().clone() {
                return Err(error);
            }
            self.kv
                .lock()
                .unwrap()
                .get(ark_store::device_registry::PEER_TRANSPORT_KEYS_KEY)
                .map_or_else(
                    || Ok(HashMap::new()),
                    |raw| serde_json::from_str(raw).map_err(|error| error.to_string()),
                )
        }
        async fn store_peer_transport_keys(
            &self,
            keys: &HashMap<String, String>,
        ) -> Result<(), String> {
            self.kv.lock().unwrap().insert(
                ark_store::device_registry::PEER_TRANSPORT_KEYS_KEY.into(),
                serde_json::to_string(keys).unwrap(),
            );
            Ok(())
        }

        async fn enroll_paired_device(
            &self,
            _local_device_id: &str,
            target_device_id: &str,
            transport_public_key: &str,
        ) -> Result<(), String> {
            let mut keys = self.load_peer_transport_keys().await?;
            if keys
                .get(target_device_id)
                .is_some_and(|existing| existing != transport_public_key)
            {
                return Err("test transport key conflict".into());
            }
            keys.insert(target_device_id.into(), transport_public_key.into());
            self.store_peer_transport_keys(&keys).await
        }

        async fn load_device_registry_events_page(
            &self,
            _: i64,
            _: Option<i64>,
            _: usize,
        ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String> {
            Err("test_registry_unsupported".into())
        }

        async fn apply_device_registry_events(
            &self,
            _: &str,
            _: &[ark_domain::device_registry::RegistryEvent],
        ) -> Result<ark_application::replication_storage::RegistryPageApply, String> {
            Err("test_registry_unsupported".into())
        }

        async fn sign_registry_handshake(
            &self,
            _: &str,
            _: &ark_domain::device_registry::RegistryHandshakeTranscript,
        ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
            Err("test_registry_unsupported".into())
        }

        async fn verify_registry_handshake(
            &self,
            _: &str,
            _: &ark_domain::device_registry::RegistryHandshakeTranscript,
            _: &ark_domain::device_registry::RegistryHandshakeProof,
        ) -> Result<(), String> {
            Err("test_registry_unsupported".into())
        }
    }
