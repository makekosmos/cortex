use crate::{checklist_ids, cortex_fixture, proposal_matches};
use yrs::types::ToJson as YrsToJson;
use yrs::{
    updates::decoder::Decode, Any, Array, ArrayRef, Doc, GetString, Map, MapRef, ReadTxn,
    StateVector, Text, Transact, Update,
};

fn yrs_seed() -> (Doc, MapRef, MapRef, ArrayRef, yrs::TextRef) {
    let fixture = cortex_fixture();
    let d = Doc::new();
    let task = d.get_or_insert_map("task");
    let props = d.get_or_insert_map("task_props");
    let checklist = d.get_or_insert_array("task_checklist");
    let content = d.get_or_insert_text("task_content");
    let project = d.get_or_insert_map("project");
    {
        let mut txn = d.transact_mut();
        task.insert(
            &mut txn,
            "title",
            Any::from(fixture["title"].as_str().unwrap()),
        );
        props.insert(
            &mut txn,
            "state.kind",
            Any::from(fixture["props_json"]["state"]["kind"].as_str().unwrap()),
        );
        props.insert(
            &mut txn,
            "priority",
            Any::from(fixture["props_json"]["priority"].as_str().unwrap()),
        );
        props.insert(&mut txn, "planning.date", Any::Null);
        props.insert(
            &mut txn,
            "project_id",
            Any::from(fixture["legacy_todo"]["project_id"].as_str().unwrap()),
        );
        for (i, id) in checklist_ids().iter().enumerate() {
            checklist.insert(&mut txn, i as u32, Any::from(id.as_str()));
        }
        content.insert(
            &mut txn,
            0,
            fixture["content_json"]["content"][0]["content"][0]["text"]
                .as_str()
                .unwrap(),
        );
        project.insert(&mut txn, "id", Any::from("project-1"));
        project.insert(&mut txn, "deleted", Any::from(false));
        project.insert(&mut txn, "name", Any::from("Current project"));
    }
    (d, task, props, checklist, content)
}

fn yrs_apply(dst: &Doc, bytes: &[u8]) {
    dst.transact_mut()
        .apply_update(Update::decode_v1(bytes).unwrap())
        .unwrap();
}

fn yrs_full_update(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

pub(crate) fn run() {
    let (base, _task, _props, _list, _text) = yrs_seed();
    let initial = yrs_full_update(&base);
    let a = Doc::new();
    let b = Doc::new();
    yrs_apply(&a, &initial);
    yrs_apply(&b, &initial);
    let task_a = a.get_or_insert_map("task");
    let props_a = a.get_or_insert_map("task_props");
    let checklist_a = a.get_or_insert_array("task_checklist");
    let text_a = a.get_or_insert_text("task_content");
    let task_b = b.get_or_insert_map("task");
    let props_b = b.get_or_insert_map("task_props");
    let checklist_b = b.get_or_insert_array("task_checklist");
    let text_b = b.get_or_insert_text("task_content");
    let project_a = a.get_or_insert_map("project");
    let project_b = b.get_or_insert_map("project");
    {
        let mut txn = a.transact_mut();
        task_a.insert(&mut txn, "title", Any::from("Canonical task carefully"));
        props_a.insert(&mut txn, "state.kind", Any::from("done"));
        props_a.insert(&mut txn, "priority", Any::from("high"));
        text_a.insert(&mut txn, 5, " carefully");
        checklist_a.insert(&mut txn, 1, Any::from("c3"));
        project_a.insert(&mut txn, "deleted", Any::from(true));
    }
    let token = a.transact().state_vector();
    {
        let mut txn = b.transact_mut();
        task_b.insert(&mut txn, "title", Any::from("Canonical task soon"));
        props_b.insert(&mut txn, "planning.date", Any::from("2026-10-20"));
        props_b.insert(&mut txn, "priority", Any::from("urgent"));
        text_b.insert(&mut txn, 5, " soon");
        checklist_b.insert(&mut txn, 1, Any::from("c4"));
        project_b.insert(&mut txn, "name", Any::from("Renamed project"));
        props_b.insert(&mut txn, "project_id", Any::from("project-1"));
    }
    let ua = yrs_full_update(&a);
    let ub = yrs_full_update(&b);
    let order_ab = Doc::new();
    let order_ba = Doc::new();
    yrs_apply(&order_ab, &initial);
    yrs_apply(&order_ab, &ua);
    yrs_apply(&order_ab, &ub);
    yrs_apply(&order_ba, &initial);
    yrs_apply(&order_ba, &ub);
    yrs_apply(&order_ba, &ua);
    assert_eq!(
        order_ab.transact().state_vector(),
        order_ba.transact().state_vector()
    );
    {
        let props_ab = order_ab.get_or_insert_map("task_props");
        let list_ab = order_ab.get_or_insert_array("task_checklist");
        let txn_ab = order_ab.transact();
        assert!(format!("{:?}", props_ab.get(&txn_ab, "state.kind")).contains("done"));
        assert!(format!("{:?}", props_ab.get(&txn_ab, "planning.date")).contains("2026-10-20"));
        let list = list_ab.to_json(&txn_ab);
        assert!(format!("{:?}", list).contains("c3") && format!("{:?}", list).contains("c4"));
    }
    yrs_apply(&a, &ub);
    yrs_apply(&a, &ub);
    yrs_apply(&b, &ua);
    let concurrent_priority = {
        let txn = a.transact();
        format!("{:?}", props_a.get(&txn, "priority"))
    };
    let stale_token = a.transact().state_vector();
    assert_eq!(
        stale_token,
        b.transact().state_vector(),
        "replicas should converge before proposal validation"
    );
    {
        let mut txn = a.transact_mut();
        props_a.insert(&mut txn, "priority", Any::from("low"));
    }
    assert_ne!(
        token, stale_token,
        "merged state differs from branch proposal context"
    );
    assert!(
        !proposal_matches(&stale_token, &a.transact().state_vector()),
        "later conflict value invalidates token"
    );
    let txn = a.transact();
    let merged_text = text_a.get_string(&txn);
    let conflict_variants_accessible = false;
    assert!(merged_text.contains("carefully") && merged_text.contains("soon"));
    assert!(task_a.get(&txn, "title").is_some());
    assert!(format!("{:?}", project_a.get(&txn, "deleted")).contains("Bool(true)"));
    assert!(format!("{:?}", project_a.get(&txn, "name")).contains("Renamed project"));
    assert!(format!("{:?}", props_a.get(&txn, "project_id")).contains("project-1"));
    let checklist_value = checklist_a.to_json(&txn);
    assert!(
        format!("{:?}", checklist_value).contains("c3")
            && format!("{:?}", checklist_value).contains("c4")
    );
    drop(txn);
    let bytes = yrs_full_update(&a);
    let ancestor = Doc::new();
    yrs_apply(&ancestor, &initial);
    let ancestor_text = ancestor.get_or_insert_text("task_content");
    let ancestor_txn = ancestor.transact();
    assert_eq!(ancestor_text.get_string(&ancestor_txn), "hello");
    drop(ancestor_txn);
    let roundtrip = Doc::new();
    yrs_apply(&roundtrip, &bytes);
    let roundtrip_text = roundtrip.get_or_insert_text("task_content");
    let roundtrip_txn = roundtrip.transact();
    assert_eq!(roundtrip_text.get_string(&roundtrip_txn), merged_text);
    println!(
        concat!(
            "yrs version=0.28.0 bytes={} concurrent_priority={} text={:?} checklist={:?} ",
            "conflict_variants_accessible={} deleted_with_updated_project_name=true ",
            "dangling_project_link=true base_snapshot_readable=true order_converged=true ",
            "duplicate_update=ok roundtrip=ok stale_conflict_token_rejected=true"
        ),
        bytes.len(),
        concurrent_priority,
        merged_text,
        checklist_value,
        conflict_variants_accessible
    );
}
