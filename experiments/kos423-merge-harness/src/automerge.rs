use crate::{checklist_ids, cortex_fixture, proposal_matches};
use automerge::{
    transaction::Transactable, AutoCommit, ObjId, ObjType, ReadDoc, ScalarValue, ROOT,
};

fn auto_seed() -> (AutoCommit, ObjId, ObjId, ObjId, ObjId, ObjId) {
    let fixture = cortex_fixture();
    let mut d = AutoCommit::new();
    let task = d.put_object(&ROOT, "task", ObjType::Map).unwrap();
    let props = d.put_object(&task, "props", ObjType::Map).unwrap();
    d.put(&task, "title", fixture["title"].as_str().unwrap())
        .unwrap();
    d.put(
        &props,
        "state.kind",
        fixture["props_json"]["state"]["kind"].as_str().unwrap(),
    )
    .unwrap();
    d.put(
        &props,
        "priority",
        fixture["props_json"]["priority"].as_str().unwrap(),
    )
    .unwrap();
    d.put(&props, "planning.date", ScalarValue::Null).unwrap();
    d.put(
        &props,
        "project_id",
        fixture["legacy_todo"]["project_id"].as_str().unwrap(),
    )
    .unwrap();
    let checklist = d.put_object(&props, "checklist", ObjType::List).unwrap();
    for (i, id) in checklist_ids().iter().enumerate() {
        d.insert(&checklist, i, id.as_str()).unwrap();
    }
    let content = d.put_object(&task, "content", ObjType::Text).unwrap();
    d.splice_text(
        &content,
        0,
        0,
        fixture["content_json"]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let project = d.put_object(&ROOT, "project", ObjType::Map).unwrap();
    d.put(&project, "id", "project-1").unwrap();
    d.put(&project, "deleted", false).unwrap();
    d.put(&project, "name", "Current project").unwrap();
    d.commit();
    (d, task, props, checklist, content, project)
}

pub(crate) fn run() {
    let (mut base, _task, props, checklist, content, project) = auto_seed();
    let base_heads = base.get_heads();
    let mut a = base.fork();
    let mut b = base.fork();
    a.put(&props, "state.kind", "done").unwrap();
    a.put(&props, "priority", "high").unwrap();
    a.put(&props, "title", "Canonical task carefully").unwrap();
    a.splice_text(&content, 5, 0, " carefully").unwrap();
    a.insert(&checklist, 1, "c3").unwrap();
    a.put(&project, "deleted", true).unwrap();
    b.put(&props, "planning.date", "2026-10-20").unwrap();
    b.put(&props, "priority", "urgent").unwrap();
    b.put(&props, "title", "Canonical task soon").unwrap();
    b.splice_text(&content, 5, 0, " soon").unwrap();
    b.insert(&checklist, 1, "c4").unwrap();
    b.put(&project, "name", "Renamed project").unwrap();
    b.put(&props, "project_id", "project-1").unwrap();
    let mut order_ab = base.fork();
    let mut a_copy = a.clone();
    let mut b_copy = b.clone();
    order_ab.merge(&mut a_copy).unwrap();
    order_ab.merge(&mut b_copy).unwrap();
    let mut order_ba = base.fork();
    let mut b_copy = b.clone();
    let mut a_copy = a.clone();
    order_ba.merge(&mut b_copy).unwrap();
    order_ba.merge(&mut a_copy).unwrap();
    assert_eq!(
        order_ab.text(&content).unwrap(),
        order_ba.text(&content).unwrap()
    );
    assert_eq!(
        order_ab
            .get(&props, "state.kind")
            .unwrap()
            .unwrap()
            .0
            .as_str(),
        Some("done")
    );
    assert_eq!(
        order_ab
            .get(&props, "planning.date")
            .unwrap()
            .unwrap()
            .0
            .as_str(),
        Some("2026-10-20")
    );
    let list_ids = |doc: &mut AutoCommit| {
        (0..doc.length(&checklist))
            .map(|i| doc.get(&checklist, i).unwrap().unwrap().0.to_string())
            .collect::<Vec<_>>()
    };
    let ids_ab = list_ids(&mut order_ab);
    let ids_ba = list_ids(&mut order_ba);
    assert_eq!(ids_ab, ids_ba);
    assert!(ids_ab.iter().any(|id| id.contains("c3")));
    assert!(ids_ab.iter().any(|id| id.contains("c4")));
    assert_eq!(order_ab.get_all(&props, "priority").unwrap().len(), 2);
    a.merge(&mut b).unwrap();
    a.merge(&mut b).unwrap();
    b.merge(&mut a).unwrap();
    assert_eq!(
        a.get_at(&props, "priority", &base_heads)
            .unwrap()
            .unwrap()
            .0
            .as_str(),
        Some("medium")
    );
    let priority_conflicts = a.get_all(&props, "priority").unwrap().len();
    let title_conflicts = a.get_all(&props, "title").unwrap().len();
    assert_eq!(
        priority_conflicts, 2,
        "Automerge should expose both scalar variants"
    );
    assert_eq!(title_conflicts, 2);
    let merged_text = a.text(&content).unwrap();
    assert!(merged_text.contains("carefully") && merged_text.contains("soon"));
    assert!(a.length(&checklist) >= 3);
    assert_eq!(
        a.get(&project, "deleted").unwrap().unwrap().0.as_bool(),
        Some(true)
    );
    assert_eq!(
        a.get(&props, "project_id").unwrap().unwrap().0.as_str(),
        Some("project-1")
    );
    let stale_token = a.get_heads();
    a.put(&props, "priority", "low").unwrap();
    assert!(
        !proposal_matches(&stale_token, &a.get_heads()),
        "new conflict variant invalidates proposal token"
    );
    let bytes = a.save();
    let loaded = AutoCommit::load(&bytes).unwrap();
    assert_eq!(loaded.text(&content).unwrap(), merged_text);
    println!(
        concat!(
            "automerge version={} bytes={} priority_variants={} title_variants={} text={:?} ",
            "checklist_len={} deleted_with_updated_project_name=true dangling_project_link=true ",
            "base_revision_readable=true order_converged=true duplicate_merge=ok roundtrip=ok ",
            "stale_conflict_token_rejected=true"
        ),
        "0.12.0",
        bytes.len(),
        priority_conflicts,
        title_conflicts,
        merged_text,
        loaded.length(&checklist)
    );
}
