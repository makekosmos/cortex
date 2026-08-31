#[allow(clippy::panic)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        package_manifest::{PermissionRequest, TargetOs, TargetRuntime},
        package_trust::{DetachedSignature, KeyTransitionDocument, PackageRevocation},
    };
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::Write};
    use tempfile::tempdir;
    use zip::write::FileOptions;

    fn key(seed: u8, id: &str) -> (SigningKey, TrustedKey) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let public_key = STANDARD.encode(signing.verifying_key().as_bytes());
        (
            signing,
            TrustedKey {
                key_id: id.into(),
                public_key,
            },
        )
    }

    fn trust() -> (TrustStore, SigningKey, SigningKey) {
        let (root_signing, root) = key(1, "root");
        let (release_signing, release) = key(2, "release-1");
        (
            TrustStore::new(root, vec![release]).expect("test trust"),
            root_signing,
            release_signing,
        )
    }

    fn manifest() -> ManifestV2 {
        ManifestV2 {
            schema_version: 2,
            id: "com.kosmos.demo".into(),
            name: "Demo".into(),
            description: None,
            version: "1.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::KosmosHost,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
                entrypoint: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![],
                defines: vec![],
                mappings: vec![],
            },
            integration: None,
        }
    }

    fn legacy_manifest() -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "com.kosmos.demo".into(),
            name: "Demo".into(),
            version: "1.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            publisher: "kosmos".into(),
            permissions: vec![],
        }
    }

    fn manifest_v2_with_canonical_access() -> ManifestV2 {
        ManifestV2 {
            schema_version: 2,
            id: "com.kosmos.demo".into(),
            name: "Demo v2".into(),
            description: None,
            version: "2.0.0".into(),
            kind: PackageKind::App,
            engine_api: ">=1.0.0".into(),
            entrypoint: "index.html".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::KosmosHost,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
                entrypoint: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![crate::package_manifest::DataAccessRule {
                    type_id: "com.kosmos.note".into(),
                    versions: "^1.0.0".into(),
                    actions: vec![crate::package_manifest::DataAction::Read],
                    fields: crate::package_manifest::FieldAccess {
                        read: vec!["props.description".into()],
                        write: vec![],
                    },
                    relations: Some(crate::package_manifest::RelationAccess {
                        read: vec!["related".into()],
                        write: vec![],
                    }),
                }],
                defines: vec![],
                mappings: vec![],
            },
            integration: None,
        }
    }

    fn catalog(sequence: u64, hash: String, size: u64, expires_at: &str) -> CatalogDocument {
        CatalogDocument {
            schema_version: 1,
            sequence,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: expires_at.into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(manifest()),
                archive_url: "https://packages.kosmos.dev/demo.kspkg".into(),
                sha256: hash,
                size,
            }],
        }
    }

    fn signed<T: Serialize>(
        document: &T,
        key_id: &str,
        key: &SigningKey,
    ) -> (Vec<u8>, SignatureSet) {
        let bytes = serde_json::to_vec(document).expect("test json");
        let signature = DetachedSignature {
            key_id: key_id.into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(key.sign(&bytes).to_bytes()),
        };
        (
            bytes,
            SignatureSet {
                schema_version: 1,
                signatures: vec![signature],
            },
        )
    }

    fn archive(root: &Path) -> (PathBuf, String, u64) {
        archive_with_versioned_manifest(root, &VersionedManifest::V2(manifest()))
    }

    fn archive_with_manifest(
        root: &Path,
        package_manifest: &PackageManifest,
    ) -> (PathBuf, String, u64) {
        let path = root.join("demo.kspkg");
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        zip.start_file("index.html", FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(b"ok").expect("entrypoint write");
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    fn archive_with_versioned_manifest(
        root: &Path,
        package_manifest: &VersionedManifest,
    ) -> (PathBuf, String, u64) {
        let path = root.join("demo-v2.kspkg");
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        zip.start_file(package_manifest.entrypoint(), FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(b"ok").expect("entrypoint write");
        if let Some(icon) = package_manifest.icon() {
            zip.start_file(icon, FileOptions::default())
                .expect("icon entry");
            zip.write_all(b"icon").expect("icon write");
        }
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    #[test]
    fn package_summary_exposes_a_verified_icon_path() {
        let dir = tempdir().expect("temp dir");
        let mut manifest = manifest();
        manifest.icon = Some("icon.ico".into());
        let expected = VersionedManifest::V2(manifest);
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &expected);
        let store = PackageStore::new(dir.path().join("packages")).expect("store");
        let package = store
            .install_versioned(&archive, size, &hash, &expected, 1)
            .expect("install");

        let listed = summary(package, None, &store);
        assert_eq!(listed.name, "Demo");
        let icon = listed.icon_path.expect("icon path");
        assert!(icon.ends_with("icon.ico"));
        assert!(!icon.starts_with(r"\\?\"));
    }

    #[test]
    fn development_app_install_uses_the_validated_archive_manifest() {
        let dir = tempdir().expect("temp dir");
        let manifest = VersionedManifest::V2(manifest_v2_with_canonical_access());
        let (archive, _, _) = archive_with_versioned_manifest(dir.path(), &manifest);
        let (trust_store, _, _) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");

        let installed = service
            .install_development_app_from_path("com.kosmos.demo", "2.0.0", &archive)
            .expect("development install");

        assert!(installed.enabled);
        assert_eq!(installed.id, "com.kosmos.demo");
        assert_eq!(installed.version, "2.0.0");
    }

    fn archive_with_versioned_manifest_and_documents(
        root: &Path,
        package_manifest: &VersionedManifest,
        documents: &[(&str, &[u8])],
    ) -> (PathBuf, String, u64) {
        let path = root.join("demo-defined.kspkg");
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        for (name, bytes) in documents {
            zip.start_file(*name, FileOptions::default())
                .expect("definition entry");
            zip.write_all(bytes).expect("definition write");
        }
        zip.start_file("index.html", FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(b"ok").expect("entrypoint write");
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    #[cfg(windows)]
    fn archive_bridge_binary(
        root: &Path,
        package_manifest: &VersionedManifest,
    ) -> (PathBuf, String, u64) {
        let path = root.join("bridge.kspkg");
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/debug/ark-markdown-bridge.exe");
        assert!(
            binary.is_file(),
            "build ark-markdown-bridge before this test: {binary:?}"
        );
        let file = File::create(&path).expect("archive file");
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("manifest.json", FileOptions::default())
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(package_manifest).expect("manifest json"))
            .expect("manifest write");
        zip.start_file(package_manifest.entrypoint(), FileOptions::default())
            .expect("entrypoint entry");
        zip.write_all(&fs::read(binary).expect("bridge binary"))
            .expect("entrypoint write");
        zip.finish().expect("archive finish");
        let bytes = fs::read(&path).expect("archive bytes");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        (path, hash, bytes.len() as u64)
    }

    pub(crate) fn enabled_app_service(dir: &Path) -> (PackageService, PathBuf, String) {
        let (archive, hash, size) = archive(dir);
        let (trust, _, release) = trust();
        let service = PackageService::open_with_trust(dir, trust).expect("service");
        let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .expect("install");
        service.enable("com.kosmos.demo", "1.0.0").expect("enable");
        (service, archive, hash)
    }

    pub(crate) fn enabled_filesystem_app_service(dir: &Path) -> PackageService {
        let mut package_manifest = manifest();
        package_manifest.permissions.push(PermissionRequest {
            capability: "filesystem.write".into(),
            scopes: vec![],
        });
        let versioned = VersionedManifest::V2(package_manifest);
        let (archive, hash, size) = archive_with_versioned_manifest(dir, &versioned);
        let (trust, _, release) = trust();
        let service = PackageService::open_with_trust(dir, trust).expect("service");
        let mut doc = catalog(1, hash, size, "2030-01-01T00:00:00Z");
        doc.packages[0].manifest = versioned;
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .expect("install");
        service.enable("com.kosmos.demo", "1.0.0").expect("enable");
        service
    }

    #[tokio::test]
    async fn uninstall_preserves_package_state_for_reinstall() {
        let dir = tempdir().expect("temp dir");
        let (service, _, _) = enabled_app_service(dir.path());
        let state = service
            .storage_root()
            .join("package-state/com.kosmos.demo/state.json");
        fs::create_dir_all(state.parent().expect("state parent")).expect("state directory");
        fs::write(&state, b"persist across reinstall").expect("state write");

        service
            .uninstall_with_worker_stop("com.kosmos.demo", "1.0.0")
            .await
            .expect("uninstall");

        assert_eq!(
            fs::read(state).expect("retained package state"),
            b"persist across reinstall"
        );
    }

    #[test]
    fn missing_compile_time_trust_fails_closed_without_blocking_engine() {
        let dir = tempdir().expect("tempdir");
        let service =
            PackageService::from_parts(dir.path().join("packages"), None).expect("service");
        assert!(!service.trust_summary().configured);
        assert_eq!(
            service.trust_summary().fault_code.as_deref(),
            Some("package_trust_unavailable")
        );
    }

    #[test]
    fn production_open_uses_pinned_trust() {
        let dir = tempdir().expect("tempdir");
        let summary = PackageService::open(dir.path())
            .expect("service")
            .trust_summary();
        assert!(summary.configured);
        assert_eq!(summary.trusted_release_keys, 1);
        assert_eq!(summary.revoked_release_keys, 0);
    }

    #[test]
    fn signed_catalog_applies_and_replay_or_tamper_fails() {
        let dir = tempdir().expect("tempdir");
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        assert_eq!(
            service
                .apply_catalog(&bytes, signatures.clone())
                .expect("catalog")
                .sequence,
            1
        );
        let app_catalog = service
            .catalog_packages(Some(&PackageKind::App))
            .expect("app catalog");
        assert_eq!(app_catalog.len(), 1);
        assert_eq!(app_catalog[0].archive_size, 1);
        assert!(service
            .catalog_packages(Some(&PackageKind::Source))
            .expect("source catalog")
            .is_empty());
        assert!(matches!(
            service.apply_catalog(&bytes, signatures),
            Err(PackageError::Trust(TrustError::Replay))
        ));
        let mut tampered = bytes;
        tampered[0] ^= 1;
        let (_, signatures) = signed(&doc, "release-1", &release);
        assert!(service.apply_catalog(tampered, signatures).is_err());
    }

    #[test]
    fn expired_or_revoked_catalog_clears_cache_but_accepts_fresh_catalog() {
        let dir = tempdir().expect("tempdir");
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let mut expired = catalog(1, "a".repeat(64), 1, "2025-01-01T00:00:00Z");
        expired.issued_at = "2024-01-01T00:00:00Z".into();
        let (expired_bytes, expired_signatures) = signed(&expired, "release-1", &release);
        assert!(service
            .apply_catalog(&expired_bytes, expired_signatures)
            .is_err());
        let (release_two, release_two_key) = key(3, "release-2");
        let transition = KeyTransitionDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            old_key_id: "release-1".into(),
            new_key: release_two_key,
        };
        let (transition_bytes, mut transition_signatures) =
            signed(&transition, "release-1", &release);
        transition_signatures.signatures.push(DetachedSignature {
            key_id: "release-2".into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(release_two.sign(&transition_bytes).to_bytes()),
        });
        service
            .apply_transition(&transition_bytes, transition_signatures)
            .expect("transition");
        let valid = catalog(2, "b".repeat(64), 1, "2030-01-01T00:00:00Z");
        let (valid_bytes, valid_signatures) = signed(&valid, "release-1", &release);
        service
            .apply_catalog(&valid_bytes, valid_signatures)
            .expect("valid catalog");
        let revoke = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec!["release-1".into()],
            revoked_packages: vec![],
        };
        let (revoke_bytes, revoke_signatures) = signed(&revoke, "root", &root);
        service
            .apply_revocations(&revoke_bytes, revoke_signatures)
            .expect("revocation");
        assert!(service.catalog_summary().is_none());
        assert_eq!(
            service.trust_summary().fault_code.as_deref(),
            Some("catalog_unavailable")
        );
        let fresh = catalog(3, "c".repeat(64), 1, "2030-01-01T00:00:00Z");
        let (fresh_bytes, fresh_signatures) = signed(&fresh, "release-2", &release_two);
        assert_eq!(
            service
                .apply_catalog(fresh_bytes, fresh_signatures)
                .expect("fresh catalog")
                .sequence,
            3
        );
    }

    #[test]
    fn transition_and_revocation_replay_are_rejected() {
        let dir = tempdir().expect("tempdir");
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let (release_two, release_two_key) = key(3, "release-2");
        let transition = KeyTransitionDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            old_key_id: "release-1".into(),
            new_key: release_two_key,
        };
        let (bytes, mut signatures) = signed(&transition, "release-1", &release);
        signatures.signatures.push(DetachedSignature {
            key_id: "release-2".into(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(release_two.sign(&bytes).to_bytes()),
        });
        service
            .apply_transition(&bytes, signatures.clone())
            .expect("transition");
        assert!(service.apply_transition(&bytes, signatures).is_err());
        let revocation = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec!["release-1".into()],
            revoked_packages: vec![],
        };
        let (bytes, signatures) = signed(&revocation, "root", &root);
        service
            .apply_revocations(&bytes, signatures.clone())
            .expect("revocation");
        assert!(service.apply_revocations(&bytes, signatures).is_err());
    }

    #[test]
    fn catalog_bound_archive_installs_and_enable_refuses_hash_mismatch() {
        let dir = tempdir().expect("tempdir");
        let (archive, hash, size) = archive(dir.path());
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        assert_eq!(
            service
                .install_from_path("com.kosmos.demo", "1.0.0", &archive)
                .expect("install")
                .id,
            "com.kosmos.demo"
        );
        service.enable("com.kosmos.demo", "1.0.0").expect("enable");

        let replacement = catalog(2, "c".repeat(64), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&replacement, "release-1", &release);
        service
            .apply_catalog(bytes, signatures)
            .expect("replacement catalog");
        assert!(service.enable("com.kosmos.demo", "1.0.0").is_err());
    }

    #[test]
    fn summaries_contain_no_sensitive_package_fields() {
        let dir = tempdir().expect("tempdir");
        let (trust, _, _) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust).expect("service");
        let json = serde_json::to_string(&service.list().expect("list")).expect("json");
        for forbidden in [
            "sha256",
            "hash",
            "entrypoint",
            "manifest",
            "signature",
            "public_key",
            "path",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn root_signed_exact_revocation_marks_installed_package() {
        let dir = tempdir().expect("tempdir");
        let (archive, hash, size) = archive(dir.path());
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let doc = catalog(1, hash.clone(), size, "2030-01-01T00:00:00Z");
        let (bytes, signatures) = signed(&doc, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path("com.kosmos.demo", "1.0.0", archive)
            .expect("install");
        let revoke = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec![],
            revoked_packages: vec![PackageRevocation {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                sha256: hash,
            }],
        };
        let (bytes, signatures) = signed(&revoke, "root", &root);
        service
            .apply_revocations(&bytes, signatures)
            .expect("revoke");
        assert!(service.list().expect("list").packages[0].revoked);
        let (fresh_trust, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), fresh_trust).expect("restart");
        assert!(restarted.list().expect("restarted list").packages[0].revoked);
    }

    #[test]
    fn app_launch_asset_lifecycle_and_immutable_tamper_boundary() {
        let dir = tempdir().expect("tempdir");
        let (service, _archive, hash) = enabled_app_service(dir.path());
        let launch = service.launch_app("com.kosmos.demo", None).expect("launch");
        assert_eq!(launch.package.hash, hash);
        assert_eq!(
            service
                .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
                .expect("asset"),
            b"ok"
        );
        assert!(service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "../index.html")
            .is_err());
        service
            .disable("com.kosmos.demo", "1.0.0")
            .expect("disable");
        assert!(service.launch_app("com.kosmos.demo", None).is_err());
        assert!(service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
            .is_err());

        let tamper_dir = tempdir().expect("tamper tempdir");
        let (service, _archive, hash) = enabled_app_service(tamper_dir.path());
        let blob = tamper_dir
            .path()
            .join("packages/blobs")
            .join(format!("{hash}.kspkg"));
        let mut bytes = fs::read(&blob).expect("blob");
        bytes[0] ^= 1;
        fs::write(blob, bytes).expect("tamper");
        assert!(service
            .read_app_asset("com.kosmos.demo", "1.0.0", &hash, "index.html")
            .is_err());
    }

    #[test]
    fn v2_grants_compile_from_canonical_registry_and_survive_restart() {
        let dir = tempdir().expect("tempdir");
        let manifest = manifest_v2_with_canonical_access();
        let versioned = VersionedManifest::V2(manifest.clone());
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
        let expected_hash = hash.clone();
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: versioned,
                archive_url: "https://packages.kosmos.dev/demo-v2.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        service
            .install_from_path(&manifest.id, &manifest.version, &archive)
            .expect("install");
        let listing = service
            .store_installed_listings()
            .expect("installed listing");
        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].effective_grants.len(), 1);
        assert_eq!(listing[0].effective_grants[0].type_id, "com.kosmos.note");
        assert_eq!(
            listing[0].effective_grants[0].fields_read,
            ["props.description"]
        );
        assert_eq!(listing[0].effective_grants[0].relations_read, ["related"]);
        service
            .enable(&manifest.id, &manifest.version)
            .expect("enable");
        assert_eq!(
            service
                .launch_app(&manifest.id, Some(&manifest.version))
                .expect("launch")
                .package
                .hash,
            expected_hash
        );

        drop(service);
        let (trust_store, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), trust_store).expect("restart");
        let listing = restarted
            .store_installed_listings()
            .expect("restarted listing");
        assert_eq!(listing[0].effective_grants[0].type_id, "com.kosmos.note");
        assert!(restarted
            .launch_app(&manifest.id, Some(&manifest.version))
            .is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn package_definitions_are_archive_bound_registered_in_ark_and_survive_uninstall() {
        let dir = tempdir().expect("tempdir");
        let mut manifest = manifest_v2_with_canonical_access();
        manifest.id = "com.kosmos.demo.definitions".into();
        manifest.name = "Defined v2".into();
        manifest.version = "1.0.0".into();
        manifest.data.access = vec![crate::package_manifest::DataAccessRule {
            type_id: "com.kosmos.demo.definitions.journal".into(),
            versions: "^1.0.0".into(),
            actions: vec![crate::package_manifest::DataAction::Read],
            fields: crate::package_manifest::FieldAccess {
                read: vec!["props.body".into()],
                write: vec![],
            },
            relations: Some(crate::package_manifest::RelationAccess {
                read: vec!["related".into()],
                write: vec![],
            }),
        }];
        manifest.data.defines = vec![crate::package_manifest::DefinitionReference {
            type_id: "com.kosmos.demo.definitions.journal".into(),
            version: "1.0.0".into(),
            schema: "schemas/journal.schema.json".into(),
            content_contract: Some("schemas/journal.content.json".into()),
            relations: Some("schemas/journal.relations.json".into()),
        }];
        manifest.validate().expect("definition manifest");
        let versioned = VersionedManifest::V2(manifest.clone());
        let documents = [
            (
                "schemas/journal.schema.json",
                br#"{"type":"object","properties":{"body":{"type":"string"}}}"# as &[u8],
            ),
            ("schemas/journal.content.json", br#"{"kind":"text"}"#),
            ("schemas/journal.relations.json", br#"[{"type":"related"}]"#),
        ];
        let (archive, hash, size) =
            archive_with_versioned_manifest_and_documents(dir.path(), &versioned, &documents);
        let docs = definition_documents_from_archive(&archive, &manifest).expect("documents");
        let probe = crate::package_registration::PackageRegistrationRegistry::open(
            dir.path().join("probe-definitions"),
        )
        .expect("probe registry");
        probe
            .validate_manifest(&manifest, &docs)
            .expect("definition contract");
        let (trust_store, _, release) = trust();
        let service = std::sync::Arc::new(
            PackageService::open_with_trust(dir.path(), trust_store).expect("service"),
        );
        let ark = std::sync::Arc::new(
            crate::ark_host::ArkHost::spawn(
                &crate::ark_host::resolve_ark_core_rpc_path().expect("ark-core-rpc binary"),
                dir.path().join("ark.db").to_str().expect("db path"),
            )
            .await
            .expect("ark host"),
        );
        let dispatcher = package_definition_dispatcher(ark.clone());
        service.configure_package_definition_dispatcher(dispatcher.clone());
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: versioned,
                archive_url: "https://packages.kosmos.dev/defined.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        {
            let service = service.clone();
            let id = manifest.id.clone();
            let version = manifest.version.clone();
            tokio::task::spawn_blocking(move || service.install_from_path(&id, &version, &archive))
                .await
                .expect("install task")
                .expect("install");
        }
        assert_eq!(
            ark.request(
                "types.get",
                serde_json::json!({ "typeId": manifest.data.defines[0].type_id }),
            )
            .await
            .expect("ARK response")
            .data
            .pointer("/summary/ownerKind")
            .and_then(serde_json::Value::as_str),
            Some("package")
        );
        let listing = service.store_installed_listings().expect("listing");
        assert_eq!(
            listing[0].effective_grants[0].type_id,
            manifest.data.defines[0].type_id
        );
        assert_eq!(listing[0].effective_grants[0].fields_read, ["props.body"]);
        let persisted = dir.path().join("packages/definitions/definitions.json");
        assert!(persisted.is_file());
        service
            .uninstall(&manifest.id, &manifest.version)
            .expect("uninstall");
        assert!(
            persisted.is_file(),
            "definitions are retained after uninstall"
        );
        drop(service);
        drop(dispatcher);
        drop(ark);
        let (trust_store, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), trust_store).expect("restart");
        let ark = std::sync::Arc::new(
            crate::ark_host::ArkHost::spawn(
                &crate::ark_host::resolve_ark_core_rpc_path().expect("ark-core-rpc binary"),
                dir.path().join("ark.db").to_str().expect("db path"),
            )
            .await
            .expect("ARK restart"),
        );
        let dispatcher = package_definition_dispatcher(ark.clone());
        let definitions = fs::read_to_string(&persisted).expect("definitions");
        assert!(definitions.contains(&manifest.data.defines[0].type_id));
        restarted.configure_package_definition_dispatcher(dispatcher.clone());
        crate::engine_api::register_package_definitions(&restarted, &dispatcher)
            .await
            .expect("idempotent replay after restart");
        assert_eq!(
            ark.request(
                "types.listVersions",
                serde_json::json!({ "typeId": manifest.data.defines[0].type_id }),
            )
            .await
            .expect("ARK response")
            .data
            .as_array()
            .expect("version list")
            .len(),
            1
        );
        assert!(restarted
            .store_installed_listings()
            .expect("listing")
            .is_empty());
    }

    fn package_definition_dispatcher(
        ark: std::sync::Arc<crate::ark_host::ArkHost>,
    ) -> std::sync::Arc<crate::engine_dispatch::EngineDispatcher> {
        std::sync::Arc::new(crate::engine_dispatch::EngineDispatcher::new(
            std::sync::Arc::new(move |request| {
                let ark = ark.clone();
                Box::pin(async move {
                    let response = ark
                        .request(request.operation.as_str(), request.params)
                        .await
                        .map_err(|error| {
                            crate::engine_dispatch::DispatchError::Failed(error.to_string())
                        })?;
                    Ok(serde_json::json!({
                        "ok": response.ok,
                        "data": response.data,
                        "error": response.error,
                    }))
                })
            }),
        ))
    }

    #[tokio::test]
    async fn ark_conflict_rolls_back_to_the_enabled_package() {
        let dir = tempdir().unwrap();
        let prior = manifest_v2_with_canonical_access();
        let prior_versioned = VersionedManifest::V2(prior.clone());
        let (prior_archive, prior_hash, prior_size) =
            archive_with_versioned_manifest(dir.path(), &prior_versioned);
        let (trust_store, _, release) = trust();
        let service =
            std::sync::Arc::new(PackageService::open_with_trust(dir.path(), trust_store).unwrap());
        let initial = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: prior_versioned,
                archive_url: "https://packages.kosmos.dev/prior.kspkg".into(),
                sha256: prior_hash.clone(),
                size: prior_size,
            }],
        };
        let (bytes, signatures) = signed(&initial, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        service
            .install_from_path(&prior.id, &prior.version, &prior_archive)
            .unwrap();
        service.enable(&prior.id, &prior.version).unwrap();
        let ark = std::sync::Arc::new(
            crate::ark_host::ArkHost::spawn(
                &crate::ark_host::resolve_ark_core_rpc_path().unwrap(),
                dir.path().join("ark.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        service.configure_package_definition_dispatcher(package_definition_dispatcher(ark.clone()));
        let type_id = "com.kosmos.demo.journal";
        assert!(ark.request("types.registerPackageDefinitions", serde_json::json!({"registrations":[{
            "type_id":type_id,"name":"Foreign","schema_json":"{}","ui_schema_json":"{}","content_contract_json":"{}","relations_json":"[]","sync_policy_json":"{}","version":"1.0.0","schema_hash":"","owner_kind":"package","owner_id":"com.example.foreign","status":"active","base_type_id":null,"aliases":[],"created_at":"now"
        }]})).await.unwrap().ok);
        let mut replacement = manifest_v2_with_canonical_access();
        replacement.id = prior.id.clone();
        replacement.version = prior.version.clone();
        replacement.data.access[0].fields.read = vec!["props.title".into()];
        replacement.data.defines = vec![crate::package_manifest::DefinitionReference {
            type_id: type_id.into(),
            version: "1.0.0".into(),
            schema: "schema.json".into(),
            content_contract: None,
            relations: None,
        }];
        replacement.validate().unwrap();
        let (archive, hash, size) = archive_with_versioned_manifest_and_documents(
            dir.path(),
            &VersionedManifest::V2(replacement.clone()),
            &[("schema.json", br#"{}"#)],
        );
        let update = CatalogDocument {
            schema_version: 1,
            sequence: 2,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(replacement),
                archive_url: "https://packages.kosmos.dev/update.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        let install = {
            let service = service.clone();
            let id = prior.id.clone();
            let version = prior.version.clone();
            tokio::task::spawn_blocking(move || service.install_from_path(&id, &version, archive))
                .await
                .unwrap()
        };
        assert!(install.is_err());
        assert_eq!(
            service
                .resolve_app(&prior.id, Some(&prior.version))
                .unwrap()
                .package
                .hash,
            prior_hash
        );
        assert!(service.list().unwrap().packages[0].enabled);
        let listing = service.store_installed_listings().unwrap();
        assert_eq!(listing[0].effective_grants.len(), 1);
        assert_eq!(
            listing[0].effective_grants[0].fields_read,
            ["props.description"]
        );
        assert!(
            service
                .package_type_registrations()
                .unwrap()
                .iter()
                .all(|registration| registration.type_id != type_id),
            "a failed install must not persist the rejected definition"
        );
    }

    #[test]
    fn failed_update_restores_revoked_record_without_reenabling_it() {
        let dir = tempdir().unwrap();
        let prior = manifest_v2_with_canonical_access();
        let prior_versioned = VersionedManifest::V2(prior.clone());
        let (prior_archive, prior_hash, prior_size) =
            archive_with_versioned_manifest(dir.path(), &prior_versioned);
        let (trust_store, root, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).unwrap();
        let initial = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: prior_versioned,
                archive_url: "https://packages.kosmos.dev/prior.kspkg".into(),
                sha256: prior_hash.clone(),
                size: prior_size,
            }],
        };
        let (bytes, signatures) = signed(&initial, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        service
            .install_from_path(&prior.id, &prior.version, &prior_archive)
            .unwrap();
        let revocation = crate::package_trust::RevocationDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            revoked_release_keys: vec![],
            revoked_packages: vec![PackageRevocation {
                id: prior.id.clone(),
                version: prior.version.clone(),
                sha256: prior_hash,
            }],
        };
        let (bytes, signatures) = signed(&revocation, "root", &root);
        service.apply_revocations(&bytes, signatures).unwrap();
        let before = service.store.installed(&prior.id, &prior.version).unwrap();
        assert!(before.revoked);
        assert!(!before.enabled);

        let mut replacement = prior.clone();
        replacement.data.access[0].type_id = "com.kosmos.unknown".into();
        let replacement_versioned = VersionedManifest::V2(replacement.clone());
        let (replacement_archive, replacement_hash, replacement_size) =
            archive_with_versioned_manifest(dir.path(), &replacement_versioned);
        let update = CatalogDocument {
            schema_version: 1,
            sequence: 2,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: replacement_versioned,
                archive_url: "https://packages.kosmos.dev/replacement.kspkg".into(),
                sha256: replacement_hash,
                size: replacement_size,
            }],
        };
        let (bytes, signatures) = signed(&update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();

        assert!(service
            .install_from_path(&replacement.id, &replacement.version, replacement_archive)
            .is_err());
        assert_eq!(
            service.store.installed(&prior.id, &prior.version).unwrap(),
            before,
            "rollback must preserve revoked/enabled/timestamp/catalog metadata"
        );
    }

    #[test]
    fn restart_gate_requires_the_exact_prior_package_record() {
        let dir = tempdir().unwrap();
        let (archive, hash, size) = archive(dir.path());
        let store = PackageStore::new(dir.path().join("store")).unwrap();
        store
            .install_versioned(&archive, size, &hash, &VersionedManifest::V2(manifest()), 1)
            .unwrap();
        store.enable("com.kosmos.demo", "1.0.0").unwrap();
        let previous = store.installed("com.kosmos.demo", "1.0.0").unwrap();

        assert!(restored_package_record_matches(
            &store,
            &previous,
            &previous.id,
            &previous.version
        ));
        store.disable(&previous.id, &previous.version).unwrap();
        assert!(!restored_package_record_matches(
            &store,
            &previous,
            &previous.id,
            &previous.version
        ));
    }

    #[test]
    fn six_provider_manifests_install_with_typed_grants() {
        let dir = tempdir().expect("tempdir");
        let mut packages = Vec::new();
        let mut archives = Vec::new();
        for package in [
            "bigfrontend",
            "greatfrontend",
            "leetcode",
            "codewars",
            "hevy",
            "toggl",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../packages")
                .join(package)
                .join("manifest.json");
            let raw = fs::read_to_string(path).unwrap_or_else(|_| panic!("{package} manifest"));
            let VersionedManifest::V2(manifest) =
                PackageManifest::parse(&raw).unwrap_or_else(|_| panic!("valid {package} manifest"))
            else {
                panic!("{package} must use a v2 manifest");
            };
            let package_dir = dir.path().join(package);
            fs::create_dir_all(&package_dir).expect("package tempdir");
            let (archive, hash, size) = archive_with_versioned_manifest(
                &package_dir,
                &VersionedManifest::V2(manifest.clone()),
            );
            packages.push(CatalogEntry {
                manifest: VersionedManifest::V2(manifest.clone()),
                archive_url: format!("https://packages.kosmos.dev/{package}.kspkg"),
                sha256: hash,
                size,
            });
            archives.push((manifest.id, manifest.version, archive));
        }

        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages,
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).expect("catalog");
        for (id, version, archive) in archives {
            service
                .install_from_path(&id, &version, archive)
                .unwrap_or_else(|_| panic!("install {id}@{version}"));
        }
        let listings = service.store_installed_listings().expect("listings");
        assert_eq!(listings.len(), 6);
        assert!(listings
            .iter()
            .all(|listing| !listing.effective_grants.is_empty()));
    }

    #[test]
    fn provider_package_manifests_use_the_generic_integration_contract() {
        for package in [
            "bigfrontend",
            "greatfrontend",
            "leetcode",
            "codewars",
            "hevy",
            "toggl",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../packages")
                .join(package)
                .join("manifest.json");
            let raw = fs::read_to_string(path).unwrap_or_else(|_| panic!("{package} manifest"));
            let VersionedManifest::V2(manifest) =
                PackageManifest::parse(&raw).unwrap_or_else(|_| panic!("valid {package} manifest"))
            else {
                panic!("{package} must use a v2 manifest");
            };
            assert_eq!(manifest.kind, PackageKind::Source);
            assert!(manifest.icon.is_some(), "{package} icon");
            assert!(manifest.targets.iter().any(|target| {
                target.runtime == TargetRuntime::Worker && target.os.contains(&TargetOs::Windows)
            }));
            assert!(manifest.integration.is_some(), "{package} integration");
        }
    }

    #[test]
    fn dictation_package_manifest_has_only_the_required_engine_grants() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dictation/package.manifest.json");
        let raw = fs::read_to_string(path).expect("Dictation package manifest");
        let VersionedManifest::V2(manifest) =
            PackageManifest::parse(&raw).expect("valid Dictation manifest")
        else {
            panic!("Dictation must be a v2 package");
        };
        let grant = compile_manifest_v2(&manifest, &RegistrySnapshot::default(), "test-digest")
            .expect("compiled Dictation grant");
        assert!(grant.allows_dictation_operation("dictation.get_config"));
        assert!(grant.allows_dictation_operation("dictation.update_config"));
        assert!(!grant.allows_dictation_operation("dictation.submit_audio"));
    }

    #[test]
    fn invalid_installed_v2_contract_does_not_brick_restart() {
        let dir = tempdir().expect("tempdir");
        let mut manifest = manifest_v2_with_canonical_access();
        manifest.data.access[0].type_id = "com.kosmos.unknown".into();
        let versioned = VersionedManifest::V2(manifest.clone());
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
        let (trust_store, _, _) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        service
            .store
            .install_versioned(&archive, size, &hash, &versioned, 1)
            .expect("install fixture");
        service
            .store
            .enable(&manifest.id, &manifest.version)
            .expect("enable fixture");
        drop(service);

        let (trust_store, _, _) = trust();
        let restarted = PackageService::open_with_trust(dir.path(), trust_store).expect("restart");
        assert!(!restarted.list().expect("list").packages[0].enabled);
        assert!(restarted.store_installed_listings().expect("listing")[0]
            .effective_grants
            .is_empty());
        assert!(restarted.enable(&manifest.id, &manifest.version).is_err());
    }

    #[test]
    fn legacy_v1_package_cannot_launch_or_survive_restart_enabled() {
        let dir = tempdir().expect("tempdir");
        let legacy = legacy_manifest();
        let (archive, hash, size) = archive_with_manifest(dir.path(), &legacy);
        let (trust_store, _, release) = trust();
        let service = PackageService::open_with_trust(dir.path(), trust_store).expect("service");
        let legacy_catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V1(legacy.clone()),
                archive_url: "https://packages.kosmos.dev/demo.kspkg".into(),
                sha256: hash.clone(),
                size,
            }],
        };
        let (bytes, signatures) = signed(&legacy_catalog, "release-1", &release);
        service
            .apply_catalog(bytes, signatures)
            .expect("legacy catalog");
        assert!(service
            .install_from_path("com.kosmos.demo", "1.0.0", &archive)
            .is_err());
        assert!(service
            .store
            .install(&archive, size, &hash, &legacy, 1)
            .is_err());
        assert!(service.list().expect("list").packages.is_empty());
    }

    #[test]
    fn source_package_never_resolves_as_app() {
        let dir = tempdir().expect("tempdir");
        let mut source = manifest();
        source.kind = PackageKind::Source;
        let versioned = VersionedManifest::V2(source.clone());
        let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &versioned);
        let service = PackageService::open(dir.path()).expect("service");
        service
            .store
            .install_versioned(&archive, size, &hash, &versioned, 1)
            .expect("install source");
        assert!(service.launch_app("com.kosmos.demo", None).is_err());
    }

    #[test]
    fn bridge_config_requires_one_real_vault_and_persists_owner_state() {
        let dir = tempdir().expect("tempdir");
        let vault = dir.path().join("vault");
        fs::create_dir(&vault).expect("vault");
        let config = BridgeConfig {
            vault_root: vault.to_string_lossy().into_owned(),
            selected_types: vec!["com.kosmos.note".into()],
            editable_fields: vec!["title".into(), "body".into()],
            readonly_fields: vec!["machine_output".into()],
        };
        assert!(validate_bridge_config(&config).is_ok());
        let state = BridgeConfigState {
            configs: HashMap::from([("ark-markdown-bridge@1.0.0".into(), config)]),
        };
        let packages = dir.path().join("packages");
        fs::create_dir(&packages).expect("packages");
        write_owner_only_json(&packages.join("bridge-config.json"), &state).expect("write");
        assert_eq!(read_bridge_configs(&packages).configs, state.configs);
        assert!(validate_bridge_config(&BridgeConfig {
            vault_root: r"\\server\vault".into(),
            selected_types: vec!["note".into()],
            editable_fields: vec![],
            readonly_fields: vec![]
        })
        .is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn signed_catalog_bridge_runs_through_service() {
        use crate::{
            ark_host::{resolve_ark_core_rpc_path, ArkHost},
            package_worker_supervisor::PackageWorkerSupervisor,
        };
        use std::sync::Arc;
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir(&vault).unwrap();
        let manifest = ManifestV2 {
            schema_version: 2,
            id: "ark-markdown-bridge".into(),
            name: "ARK Markdown Bridge".into(),
            description: None,
            version: "1.0.0".into(),
            kind: PackageKind::Bridge,
            engine_api: ">=1".into(),
            entrypoint: "ark-markdown-bridge.exe".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![
                PermissionRequest {
                    capability: "ark.read".into(),
                    scopes: vec!["list_objects".into(), "get_object".into()],
                },
                PermissionRequest {
                    capability: "ark.write".into(),
                    scopes: vec!["upsert_object".into(), "external_refs.upsert".into()],
                },
                PermissionRequest {
                    capability: "filesystem.read".into(),
                    scopes: vec![],
                },
                PermissionRequest {
                    capability: "filesystem.write".into(),
                    scopes: vec![],
                },
            ],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::Worker,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
                entrypoint: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![],
                defines: vec![],
                mappings: vec![],
            },
            integration: None,
        };
        let versioned = VersionedManifest::V2(manifest.clone());
        let (archive, hash, size) = archive_bridge_binary(dir.path(), &versioned);
        let (trust, _, release) = trust();
        let mut service = PackageService::open_with_trust(dir.path(), trust).unwrap();
        let catalog = CatalogDocument {
            schema_version: 1,
            sequence: 1,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(manifest.clone()),
                archive_url: "https://packages.kosmos.dev/bridge.kspkg".into(),
                sha256: hash,
                size,
            }],
        };
        let (bytes, signatures) = signed(&catalog, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        service
            .install_from_path(&manifest.id, &manifest.version, archive)
            .unwrap();
        let ark = Arc::new(
            ArkHost::spawn(
                &resolve_ark_core_rpc_path().unwrap(),
                dir.path().join("ark.db").to_str().unwrap(),
            )
            .await
            .unwrap(),
        );
        let note_registration =
            ark_core::canonical_types::definitions::canonical_type_registrations()
                .unwrap()
                .into_iter()
                .find(|registration| registration.type_id == "com.kosmos.note")
                .unwrap();
        assert!(
            ark.request(
                "types.registerPackageDefinitions",
                serde_json::json!({"registrations":[note_registration]})
            )
            .await
            .unwrap()
            .ok
        );
        assert!(ark.request("upsert_object", serde_json::json!({"object":{"id":"service-note","typeId":"com.kosmos.note","typeVersion":"1.0.0","title":"Service note","contentJson":{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"from ark"}]}]},"propsJson":{"description":null,"extensions":{}},"createdAt":"2026-01-01T00:00:00Z","updatedAt":"2026-01-01T00:00:00Z","deletedAt":null},"device_id":"bridge-service"})).await.unwrap().ok);
        let objects = ark
            .request("list_objects", serde_json::Value::Null)
            .await
            .unwrap()
            .data;
        let note = objects
            .as_array()
            .and_then(|objects| objects.iter().find(|object| object["id"] == "service-note"))
            .expect("created note must be visible to the bridge");
        assert_eq!(note["typeId"], "com.kosmos.note");
        let supervisor = PackageWorkerSupervisor::with_ark(1, ark.clone());
        service.configure_workers(supervisor.clone(), vec![], "bridge-service".into());
        service
            .set_bridge_config(
                &manifest.id,
                &manifest.version,
                BridgeConfig {
                    vault_root: vault.to_string_lossy().into_owned(),
                    selected_types: vec!["com.kosmos.note".into()],
                    editable_fields: vec!["title".into(), "body".into()],
                    readonly_fields: vec![],
                },
            )
            .await
            .unwrap();
        if let Err(error) = service
            .set_enabled(&manifest.id, &manifest.version, true)
            .await
        {
            panic!(
                "bridge enable failed: {error:?}; diagnostics: {:?}",
                supervisor.diagnostics()
            );
        }
        let markdown = vault.join("Service note-service-note.md");
        let bridge_state = dir
            .path()
            .join("packages")
            .join("bridge-state")
            .join(&manifest.id)
            .join(&manifest.version)
            .join("state.json");
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            // Wait for durable provenance before editing the first snapshot;
            // otherwise the worker can classify the edit as initial state.
            while !markdown.exists() || !bridge_state.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "bridge did not write markdown: {:?}",
                supervisor.diagnostics(),
            )
        });
        let content = fs::read_to_string(&markdown).unwrap();
        fs::write(&markdown, content.replace("from ark", "from vault")).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            loop {
                let current = ark
                    .request("get_object", serde_json::json!({"id":"service-note"}))
                    .await
                    .unwrap();
                if current.data["contentJson"]["content"][0]["content"][0]["text"] == "from vault" {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "bridge did not sync vault edit: {:?}",
                supervisor.diagnostics()
            )
        });

        let mut replacement = manifest.clone();
        replacement.name = "ARK Markdown Bridge Updated".into();
        let (replacement_archive, replacement_hash, replacement_size) =
            archive_bridge_binary(dir.path(), &VersionedManifest::V2(replacement.clone()));
        let update = CatalogDocument {
            schema_version: 1,
            sequence: 2,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(replacement.clone()),
                archive_url: "https://packages.kosmos.dev/bridge-update.kspkg".into(),
                sha256: replacement_hash,
                size: replacement_size,
            }],
        };
        let (bytes, signatures) = signed(&update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        let updated = service
            .install_from_path_with_worker_stop(
                &replacement.id,
                &replacement.version,
                replacement_archive,
            )
            .await
            .unwrap();
        assert!(updated.enabled);
        assert_eq!(updated.worker_state, WorkerState::Running);
        assert_eq!(
            service
                .store
                .installed(&replacement.id, &replacement.version)
                .unwrap()
                .manifest
                .common_manifest()
                .name,
            replacement.name
        );

        let previous = service
            .store
            .installed(&replacement.id, &replacement.version)
            .unwrap();
        let mut broken = replacement.clone();
        broken.name = "ARK Markdown Bridge Broken Update".into();
        let (broken_archive, broken_hash, broken_size) =
            archive_bridge_binary(dir.path(), &VersionedManifest::V2(broken.clone()));
        let broken_update = CatalogDocument {
            schema_version: 1,
            sequence: 3,
            issued_at: "2029-01-01T00:00:00Z".into(),
            expires_at: "2030-01-01T00:00:00Z".into(),
            packages: vec![CatalogEntry {
                manifest: VersionedManifest::V2(broken.clone()),
                archive_url: "https://packages.kosmos.dev/bridge-broken.kspkg".into(),
                sha256: broken_hash,
                size: broken_size,
            }],
        };
        let (bytes, signatures) = signed(&broken_update, "release-1", &release);
        service.apply_catalog(bytes, signatures).unwrap();
        supervisor.test_fail_next_start();
        let failed_update = service
            .install_from_path_with_worker_stop(&broken.id, &broken.version, broken_archive)
            .await;
        assert!(
            matches!(failed_update, Err(PackageError::Worker("unavailable"))),
            "unexpected failed update result: {failed_update:?}"
        );
        assert_eq!(
            service
                .store
                .installed(&replacement.id, &replacement.version)
                .unwrap(),
            previous
        );
        assert_eq!(
            supervisor
                .health(&replacement.id, &replacement.version)
                .state,
            WorkerState::Running
        );
        assert_eq!(
            supervisor
                .diagnostics()
                .into_iter()
                .find(|worker| worker.id == replacement.id && worker.version == replacement.version)
                .and_then(|worker| worker.hash),
            Some(previous.hash.clone())
        );
        service
            .set_enabled(&manifest.id, &manifest.version, false)
            .await
            .unwrap();
        assert_eq!(
            supervisor.health(&manifest.id, &manifest.version).state,
            WorkerState::Stopped
        );
    }
}
