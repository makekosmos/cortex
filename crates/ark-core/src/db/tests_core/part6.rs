
    #[test]
    fn test_todo_with_tags_and_checklist() {
        let conn = setup_db();
        let todo = TodoItem {
            tag_ids: vec!["tag1".to_string(), "tag2".to_string()],
            checklist_items: json!([{"id": "c1", "title": "Step 1", "isCompleted": false}]),
            recurrence_rule: Some(json!({"frequency": "daily", "interval": 1})),
            ..make_todo("t1", "Complex")
        };
        upsert_todo(&conn, &todo).unwrap();

        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos[0].tag_ids, vec!["tag1", "tag2"]);
        assert!(data.todos[0].checklist_items.is_array());
        assert!(data.todos[0].recurrence_rule.is_some());
    }
