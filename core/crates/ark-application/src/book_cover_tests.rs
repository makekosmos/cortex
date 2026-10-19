use crate::book_cover::{
    canonical_book_cover_link_id, plan_local_book_cover, BookCoverError, LocalBookCoverDecision,
    LocalBookCoverFacts, LocalBookCoverWrite, BOOK_COVER_LINK_TYPE, BOOK_TYPE_ID,
    CANONICAL_VERSION, IMAGE_TYPE_ID,
};
use ark_domain::types::{ArkObject, ObjectLink};
use serde_json::json;

fn object(id: &str, type_id: &str, deleted_at: Option<&str>) -> ArkObject {
    let props_json = if type_id == BOOK_TYPE_ID {
        json!({
            "author": null, "isbn": null, "pageCount": null, "language": null,
            "publisher": null, "publishedDate": null, "sourceUrl": null, "extensions": {}
        })
    } else {
        json!({
            "fileName": null, "mimeType": null, "sizeBytes": null, "width": null,
            "height": null, "resolution": null, "altText": "", "extensions": {}
        })
    };
    ArkObject {
        id: id.into(),
        type_id: type_id.into(),
        type_version: CANONICAL_VERSION.into(),
        title: id.into(),
        content_json: json!({"type": "doc", "content": []}),
        props_json,
        created_at: "2026-10-19T00:00:00.000Z".into(),
        updated_at: "2026-10-19T00:00:00.000Z".into(),
        deleted_at: deleted_at.map(str::to_owned),
    }
}

fn write(image_id: Option<&str>) -> LocalBookCoverWrite {
    LocalBookCoverWrite {
        book_id: "book-1".into(),
        source_ref: None,
        existing_image_id: image_id.map(str::to_owned),
        alt_text: String::new(),
        device_id: "device-1".into(),
    }
}

fn facts(image_id: Option<&str>, existing_cover_links: Vec<ObjectLink>) -> LocalBookCoverFacts {
    LocalBookCoverFacts {
        book: Some(object("book-1", BOOK_TYPE_ID, None)),
        image: image_id.map(|id| object(id, IMAGE_TYPE_ID, None)),
        existing_cover_links,
        link_created_at: "2026-10-18T00:00:00.000Z".into(),
    }
}

fn link(image_id: &str) -> ObjectLink {
    ObjectLink {
        id: canonical_book_cover_link_id("book-1", image_id),
        source_object_id: "book-1".into(),
        target_object_id: image_id.into(),
        link_type: BOOK_COVER_LINK_TYPE.into(),
        created_at: "2026-10-18T00:00:00.000Z".into(),
    }
}

#[test]
fn cover_planner_preserves_noop_and_replacement_behavior() {
    assert_eq!(
        plan_local_book_cover(
            write(Some("image-1")),
            facts(Some("image-1"), vec![link("image-1")])
        )
        .expect("existing sole cover is valid"),
        LocalBookCoverDecision::Unchanged
    );

    let LocalBookCoverDecision::Change(plan) = plan_local_book_cover(
        write(Some("image-2")),
        facts(Some("image-2"), vec![link("image-1")]),
    )
    .expect("new image creates a replacement plan") else {
        panic!("replacement must change the cover")
    };
    assert_eq!(plan.deleted_link_ids, vec![link("image-1").id]);
    assert_eq!(plan.new_link, Some(link("image-2")));
}

#[test]
fn cover_planner_preserves_unsupported_source_errors() {
    let mut write = write(None);
    write.source_ref = Some("file:///cover.png".into());
    assert_eq!(
        plan_local_book_cover(write, facts(None, vec![])).unwrap_err(),
        BookCoverError {
            code: "source_requires_existing_image",
            pointer: None,
        }
    );
}
