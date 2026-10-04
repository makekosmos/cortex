use super::*;

#[test]
fn handles_are_opaque_and_resolution_requires_exact_binding() {
    let registry = PackageWorkerSecretRegistry::new();
    let handle = registry
        .issue("pkg", "1.0", 7, "api_key", "top-secret".into())
        .unwrap();
    assert_eq!(SecretHandle::parse(&handle.token()), Some(handle));
    assert_ne!(format!("{handle:?}"), "top-secret");
    assert_eq!(
        registry.resolve(&handle, "pkg", "1.0", 7, "api_key"),
        Ok("top-secret".into())
    );
    assert_eq!(
        registry.resolve(&handle, "pkg", "1.0", 7, "other"),
        Err(SecretRegistryError::OwnerMismatch)
    );
    assert_eq!(
        registry.resolve_token(&handle.token(), "pkg", "1.0", 7),
        Ok(("api_key".into(), "top-secret".into()))
    );
}

#[test]
fn revocation_is_bounded_and_scoped() {
    let registry = PackageWorkerSecretRegistry::new();
    let old = registry.issue("pkg", "1", 1, "a", "old".into()).unwrap();
    let current = registry
        .issue("pkg", "2", 2, "a", "current".into())
        .unwrap();
    let other = registry
        .issue("other", "1", 1, "a", "other".into())
        .unwrap();
    assert_eq!(registry.revoke_generation("pkg", 1), 1);
    assert_eq!(
        registry.resolve(&old, "pkg", "1", 1, "a"),
        Err(SecretRegistryError::NotFound)
    );
    assert_eq!(
        registry.resolve(&current, "pkg", "2", 2, "a"),
        Ok("current".into())
    );
    assert_eq!(registry.revoke_package("pkg"), 1);
    assert_eq!(
        registry.resolve(&other, "other", "1", 1, "a"),
        Ok("other".into())
    );
}

#[test]
fn issue_rejects_unbounded_inputs_and_capacity_overflow() {
    let registry = PackageWorkerSecretRegistry::new();
    assert_eq!(
        registry.issue("", "1", 1, "key", "secret".into()),
        Err(SecretRegistryError::InvalidBinding)
    );
    assert_eq!(
        registry.issue("pkg", "1", 1, "key", "x".repeat(MAX_SECRET_BYTES + 1)),
        Err(SecretRegistryError::SecretTooLarge)
    );
    for index in 0..MAX_SECRET_HANDLES {
        registry
            .issue("pkg", "1", 1, &format!("key-{index}"), String::new())
            .unwrap();
    }
    assert_eq!(registry.len(), MAX_SECRET_HANDLES);
    assert_eq!(
        registry.issue("pkg", "1", 1, "overflow", String::new()),
        Err(SecretRegistryError::Capacity)
    );
}

#[test]
fn record_zeroization_clears_string_bytes() {
    let mut record = SecretRecord {
        package_id: "pkg".into(),
        version: "1".into(),
        generation: 1,
        setting_key: "key".into(),
        secret: "secret".into(),
    };
    record.zeroize();
    assert_eq!(record.secret.as_bytes(), &[0; 6]);
}
