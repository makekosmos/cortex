#[test]
fn arrancador_handler_uses_only_typed_game_facade_operations() {
    let source = include_str!("../src/ws_server.rs");
    let start = source
        .find("async fn handle_arrancador_op(")
        .expect("Arrancador handler declaration");
    let rest = &source[start..];
    let end = rest.find("\nasync fn ").expect("next handler boundary");
    let handler = &rest[..end];
    for forbidden in [
        "get_object",
        "list_objects_by_type",
        "list_object_links",
        "upsert_object",
        "props_json",
        "content_json",
    ] {
        assert!(
            !handler.contains(forbidden),
            "Arrancador handler retained forbidden legacy operation: {forbidden}"
        );
    }
}
