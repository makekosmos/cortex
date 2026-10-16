use serde_json::Value;

pub(crate) fn cortex_fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/core-task.json")).unwrap()
}

pub(crate) fn checklist_ids() -> Vec<String> {
    cortex_fixture()["props_json"]["checklist"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect()
}

pub(crate) fn proposal_matches<T: Eq>(proposal_token: &T, current_token: &T) -> bool {
    proposal_token == current_token
}

mod automerge;
mod loro;
mod yrs;

fn main() {
    let fixture = cortex_fixture();
    assert_eq!(fixture["type_id"], "core.task");
    assert_eq!(fixture["type_version"], "0.1.0");
    assert_eq!(fixture["props_json"]["state"]["kind"], "open");
    assert_eq!(fixture["props_json"]["planning"]["kind"], "anytime");
    assert_eq!(fixture["props_json"]["priority"], "medium");
    assert_eq!(
        fixture["content_json"]["content"][0]["content"][0]["text"],
        "hello"
    );
    assert_eq!(checklist_ids(), vec!["c1"]);
    assert_eq!(fixture["legacy_todo"]["project_id"], "project-2");
    assert_eq!(fixture["legacy_todo"]["checklist_items"][0]["id"], "c1");
    println!(concat!(
        "fixture: core.task 0.1.0 based on ark-core canonical ingress fixture; ",
        "Tiptap leaf text and checklist IDs are extracted; merge operations remain a ",
        "standalone adapter experiment, not ark-core integration tests"
    ));
    automerge::run();
    loro::run();
    yrs::run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_task_fixture_is_cortex_shaped() {
        let fixture = cortex_fixture();
        assert_eq!(fixture["type_id"], "core.task");
        assert_eq!(fixture["type_version"], "0.1.0");
        assert_eq!(fixture["props_json"]["state"]["kind"], "open");
        assert_eq!(fixture["props_json"]["planning"]["kind"], "anytime");
        assert_eq!(
            fixture["props_json"]["planning"]["date"],
            serde_json::Value::Null
        );
        assert_eq!(fixture["props_json"]["checklist"][0]["id"], "c1");
        assert_eq!(fixture["legacy_todo"]["project_id"], "project-2");
    }

    #[test]
    fn all_three_engines_run_the_concurrent_fixture_assertions() {
        automerge::run();
        loro::run();
        yrs::run();
    }
}
