#[test]

fn equivalent_existing_link_is_reused_exactly_once() {
    let conn = db();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at) VALUES('img-1','com.kosmos.image','1.0.0','Image',",
            "'{}','{}','c','u')"
        ),
        [],
    )
    .unwrap();
    insert_generic(
        &conn,
        "book-1",
        "book_obj",
        json!({"author":"A","page_count":10,"cover_image":"img-1"}),
        None,
    );
    conn.execute(
        concat!(
            "INSERT INTO object_links(id,source_object_id,target_object_id,link_type,",
            "created_at) VALUES('existing-link','book-1','img-1','cover-image','u')"
        ),
        [],
    )
    .unwrap();
    let plan = plan_objects(&conn, "now").unwrap();
    assert!(plan.blocked.is_empty());
    apply_plan(&conn, &plan, "now").unwrap();
    let (count, id): (i64, String) = conn
        .query_row(
            concat!(
                "SELECT COUNT(*),MIN(id) FROM object_links WHERE source_object_id='book-1' ",
                "AND target_object_id='img-1' AND link_type='cover-image'"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((count, id), (1, "existing-link".into()));
}

#[test]
fn game_local_aggregate_and_book_image_quarantine_are_persisted() {
    let conn = db();
    insert_generic(
        &conn,
        "game-1",
        "game_obj",
        json!(
            {"play_status":"completed",
            "genres":["rpg"],
            "total_playtime_seconds":42,
            "exe_path":"/games/a",
            "provider":"rawg"}),
        None,
    );
    insert_generic(
        &conn,
        "image-1",
        "image_obj",
        json!({"file_name":"a.png","mime_type":"image/png","size_bytes":7,"alt_text":"a"}),
        None,
    );
    insert_generic(
        &conn,
        "book-1",
        "book_obj",
        json!({"author":"A","page_count":10,"cover_image":"https://example.invalid/cover"}),
        None,
    );
    let plan = plan_objects(&conn, "now").unwrap();
    assert!(plan.blocked.is_empty());
    apply_plan(&conn, &plan, "now").unwrap();
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM object_local_state WHERE object_id='game-1'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(
            concat!(
                "SELECT COUNT(*) FROM object_migration_quarantine WHERE object_id IN (",
                "'game-1','book-1')"
            ),
            [],
            |r| r.get(0)
        )
        .unwrap(),
        2
    );
    assert_eq!(
        conn.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM object_migration_quarantine WHERE object_id='image-1'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn late_link_collision_rolls_back_objects_and_ledger_rows() {
    let conn = db();
    insert_generic(&conn, "project-1", "project_obj", json!({}), None);
    insert_generic(
        &conn,
        "task-1",
        "task_obj",
        json!({"project_id":"project-1"}),
        None,
    );
    let collision = ark_core::canonical_types::migration_objects::stable_link_id(
        "task-1",
        "project",
        "project-1",
    );
    conn.execute(
        concat!(
            "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
            "updated_at) VALUES('other','other','{}','{}','c','u')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,",
            "created_at,updated_at) VALUES('other','other','1.0.0','other','{}','{}','c',",
            "'u')"
        ),
        [],
    )
    .unwrap();
    conn.execute(
        concat!(
            "INSERT INTO object_links(id,source_object_id,target_object_id,link_type,",
            "created_at) VALUES(?1,'other','project-1','project','x')"
        ),
        [collision],
    )
    .unwrap();
    let plan = plan_objects(&conn, "now").unwrap();
    assert!(apply_plan(&conn, &plan, "now").is_err());
    assert_eq!(
        conn.query_row::<i64, _, _>(
            concat!(
                "SELECT COUNT(*) FROM objects WHERE id IN ('project-1','task-1') AND type_id ",
                "LIKE 'com.kosmos.%'"
            ),
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM canonical_migration_items", [], |r| r
            .get(0))
            .unwrap_or(0),
        0
    );
}
