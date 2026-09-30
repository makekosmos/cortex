use ark_core::type_registry::{get_type, register_type, TypeRegistration};
use rusqlite::Connection;

fn registration() -> TypeRegistration {
    serde_json::from_str(include_str!(
        "../../../../integrations/fatsecret/type-registration.json"
    ))
    .expect("FatSecret registration must deserialize")
}

#[test]
fn fatsecret_registration_is_idempotent_and_keeps_package_identity() {
    let conn = Connection::open_in_memory().expect("open fixture database");
    ark_core::db::init_schema(&conn).expect("initialize schema");
    let registration = registration();
    register_type(&conn, &registration).expect("register package type");
    register_type(&conn, &registration).expect("repeat package registration");
    let stored = get_type(&conn, "nutrition_entry_obj", Some("1.0.0"))
        .expect("load registered type")
        .expect("type exists");
    assert_eq!(stored.summary.owner_kind, "package");
    assert_eq!(
        stored.summary.owner_id.as_deref(),
        Some("com.kosmos.integrations.fatsecret")
    );
    assert!(!stored.definition.schema_hash.is_empty());
}
