use ark_domain::canonical_type::TypeRegistration;
use ark_store::test_support::{TestDatabase, TypeMutationFixture};

fn registration() -> TypeRegistration {
    serde_json::from_str(include_str!(
        "../../../../integrations/fatsecret/type-registration.json"
    ))
    .expect("FatSecret registration must deserialize")
}

#[test]
fn fatsecret_registration_is_idempotent_and_keeps_package_identity() {
    let database = TestDatabase::create().expect("initialize fixture database");
    let fixture = TypeMutationFixture::open(database.path()).expect("open typed fixture");
    let registration = registration();
    fixture
        .register_type(&registration)
        .expect("register package type");
    fixture
        .register_type(&registration)
        .expect("repeat package registration");
    let stored = fixture
        .load_registered_type("nutrition_entry_obj", "1.0.0")
        .expect("load registered type")
        .expect("type exists");
    assert_eq!(stored.summary.owner_kind, "package");
    assert_eq!(
        stored.summary.owner_id.as_deref(),
        Some("com.kosmos.integrations.fatsecret")
    );
    assert!(!stored.definition.schema_hash.is_empty());
}
