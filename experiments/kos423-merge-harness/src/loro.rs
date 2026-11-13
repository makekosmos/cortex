use crate::{checklist_ids, cortex_fixture, proposal_matches};
use loro::{ExportMode, LoroDoc, ToJson};

fn loro_seed() -> LoroDoc {
    let fixture = cortex_fixture();
    let d = LoroDoc::new();
    let task = d.get_map("task");
    task.insert("title", fixture["title"].as_str().unwrap())
        .unwrap();
    let props = d.get_map("task_props");
    props
        .insert(
            "state.kind",
            fixture["props_json"]["state"]["kind"].as_str().unwrap(),
        )
        .unwrap();
    props
        .insert(
            "priority",
            fixture["props_json"]["priority"].as_str().unwrap(),
        )
        .unwrap();
    props
        .insert("planning.date", serde_json::Value::Null)
        .unwrap();
    props
        .insert(
            "project_id",
            fixture["legacy_todo"]["project_id"].as_str().unwrap(),
        )
        .unwrap();
    let list = d.get_list("task_checklist");
    for (i, id) in checklist_ids().iter().enumerate() {
        list.insert(i, id.as_str()).unwrap();
    }
    let text = d.get_text("task_content");
    text.insert(
        0,
        fixture["content_json"]["content"][0]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let project = d.get_map("project");
    project.insert("id", "project-1").unwrap();
    project.insert("deleted", false).unwrap();
    project.insert("name", "Current project").unwrap();
    d.commit();
    d
}

fn merge_loro(a: &LoroDoc, b: &LoroDoc) {
    let updates = b.export(ExportMode::all_updates()).unwrap();
    a.import(&updates).unwrap();
}

pub(crate) fn run() {
    let base = loro_seed();
    let base_frontiers = base.state_frontiers();
    let base_value = base.get_deep_value().to_json_value();
    let a = base.fork();
    let b = base.fork();
    let props_a = a.get_map("task_props");
    let props_b = b.get_map("task_props");
    props_a.insert("state.kind", "done").unwrap();
    props_a.insert("priority", "high").unwrap();
    a.get_map("task")
        .insert("title", "Canonical task carefully")
        .unwrap();
    a.get_text("task_content").insert(5, " carefully").unwrap();
    a.get_list("task_checklist").insert(1, "c3").unwrap();
    a.get_map("project").insert("deleted", true).unwrap();
    props_b.insert("planning.date", "2026-10-20").unwrap();
    props_b.insert("priority", "urgent").unwrap();
    b.get_map("task")
        .insert("title", "Canonical task soon")
        .unwrap();
    b.get_text("task_content").insert(5, " soon").unwrap();
    b.get_list("task_checklist").insert(1, "c4").unwrap();
    b.get_map("project")
        .insert("name", "Renamed project")
        .unwrap();
    props_b.insert("project_id", "project-1").unwrap();
    let ua = a.export(ExportMode::all_updates()).unwrap();
    let ub = b.export(ExportMode::all_updates()).unwrap();
    let order_ab = base.fork();
    order_ab.import_batch(&[ua.clone(), ub.clone()]).unwrap();
    let order_ba = base.fork();
    order_ba.import_batch(&[ub.clone(), ua.clone()]).unwrap();
    assert_eq!(
        order_ab.get_deep_value().to_json_value(),
        order_ba.get_deep_value().to_json_value()
    );
    merge_loro(&a, &b);
    merge_loro(&a, &b);
    merge_loro(&b, &a);
    let merged = a.get_deep_value().to_json_value();
    assert_eq!(b.get_deep_value().to_json_value(), merged);
    let text = a
        .get_text("task_content")
        .get_richtext_value()
        .to_json_value();
    let priority = a
        .get_map("task_props")
        .get("priority")
        .unwrap()
        .get_deep_value()
        .to_json_value();
    assert_eq!(merged["project"]["deleted"], true);
    assert_eq!(merged["project"]["name"], "Renamed project");
    assert_eq!(merged["task_props"]["project_id"], "project-1");
    assert_eq!(merged["task_props"]["state.kind"], "done");
    assert_eq!(merged["task_props"]["planning.date"], "2026-10-20");
    assert!(merged["task_checklist"].to_string().contains("c3"));
    assert!(merged["task_checklist"].to_string().contains("c4"));
    let stale_token = a.state_frontiers();
    a.get_map("task_props").insert("priority", "low").unwrap();
    assert!(!proposal_matches(&stale_token, &a.state_frontiers()));
    let merged = a.get_deep_value().to_json_value();
    assert!(text.to_string().contains("carefully") && text.to_string().contains("soon"));

    let ancestor = a.fork_at(&base_frontiers).unwrap();
    assert_eq!(ancestor.get_deep_value().to_json_value(), base_value);
    let bytes = a.export(ExportMode::Snapshot).unwrap();
    let loaded = LoroDoc::from_snapshot(&bytes).unwrap();
    assert_eq!(loaded.get_deep_value().to_json_value(), merged);
    println!(
        concat!(
            "loro version=1.16.2 bytes={} priority_winner={} text={:?} checklist={} ",
            "deleted_with_updated_project_name=true dangling_project_link=true ",
            "base_revision_readable=true order_converged=true duplicate_import=ok roundtrip=ok ",
            "stale_conflict_token_rejected=true"
        ),
        bytes.len(),
        priority,
        text,
        merged["task_checklist"]
    );
}
