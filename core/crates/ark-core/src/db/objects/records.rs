pub fn upsert_object(conn: &Connection, object: &ArkObject) -> Result<(), String> {
    if ark_store::objects::object_type_registry_exists(conn)? {
        let (canonical_type_id, canonical_version) =
            crate::type_registry::resolve_object_type_identity(
                conn,
                &object.type_id,
                Some(&object.type_version),
            )?;
        let mut canonical_object = object.clone();
        canonical_object.type_id = canonical_type_id;
        canonical_object.type_version = canonical_version;
        return ark_store::objects::upsert_object(conn, &canonical_object);
    }
    ark_store::objects::upsert_object(conn, object)
}
