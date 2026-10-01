const BOOK_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.book",
  "name": "Книга",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["book_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "author": {
        "type": ["string", "null"],
        "default": null
      },
      "isbn": {
        "type": ["string", "null"],
        "default": null
      },
      "pageCount": {
        "type": ["integer", "null"],
        "default": null,
        "minimum": 0
      },
      "language": {
        "type": ["string", "null"],
        "default": null
      },
      "publisher": {
        "type": ["string", "null"],
        "default": null
      },
      "publishedDate": {
        "type": ["string", "null"],
        "default": null,
        "format": "date"
      },
      "sourceUrl": {
        "type": ["string", "null"],
        "default": null,
        "format": "uri"
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": [
      "author",
      "isbn",
      "pageCount",
      "language",
      "publisher",
      "publishedDate",
      "sourceUrl",
      "extensions"
    ]
  },
  "uiSchema": {
    "collectionName": "Книги",
    "featuredFields": ["author"],
    "visibleFields": [
      "author",
      "isbn",
      "pageCount",
      "language",
      "publisher",
      "publishedDate",
      "sourceUrl"
    ],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": [
      "author",
      "isbn",
      "pageCount",
      "language",
      "publisher",
      "publishedDate",
      "sourceUrl"
    ],
    "headerLayout": "inline",
    "defaultLayout": "page",
    "defaultTemplateId": null
  },
  "contentContract": {
    "mediaType": "application/vnd.kosmos.richtext+json",
    "version": 1,
    "rootType": "doc",
    "allowedNodes": [
      "doc",
      "paragraph",
      "text",
      "heading",
      "bulletList",
      "orderedList",
      "listItem",
      "blockquote",
      "codeBlock",
      "hardBreak",
      "horizontalRule",
      "taskList",
      "taskItem",
      "image"
    ],
    "allowedMarks": ["bold", "italic", "strike", "code", "link", "underline"],
    "attributes": {
      "heading": {
        "level": {
          "type": "integer",
          "minimum": 1,
          "maximum": 6
        }
      },
      "orderedList": {
        "start": {
          "type": "integer",
          "minimum": 1,
          "default": 1
        }
      },
      "codeBlock": {
        "language": {
          "type": ["string", "null"],
          "default": null
        }
      },
      "link": {
        "href": {
          "type": "string"
        },
        "target": {
          "type": ["string", "null"],
          "default": null
        },
        "rel": {
          "type": ["string", "null"],
          "default": null
        },
        "class": {
          "type": ["string", "null"],
          "default": null
        }
      },
      "image": {
        "src": {
          "type": ["string", "null"],
          "default": null
        },
        "alt": {
          "type": ["string", "null"],
          "default": null
        },
        "title": {
          "type": ["string", "null"],
          "default": null
        }
      },
      "taskItem": {
        "checked": {
          "type": "boolean",
          "default": false
        }
      }
    }
  },
  "relations": [
    {
      "type": "cover-image",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.image",
      "min": 0,
      "max": 1
    },
    {
      "type": "tag",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.tag",
      "min": 0,
      "max": null
    },
    {
      "type": "author-person",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.person",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "8ce61b253c590aa2b54b00bac4854c8b0e92469bb916e58ebc9fccd7070b12a0"
}"###;

/// Return the ordered first-party registrations.
///
/// A type may appear more than once: stored objects keep the version they
/// were written with, so every canonical version must stay resolvable.
/// Ordering is significant — later registrations of the same type supersede
/// earlier ones where only a type id is known (alias resolution, compat
/// mapping), so a new version is always appended after its predecessor.
pub fn canonical_type_registrations() -> Result<Vec<TypeRegistration>, String> {
    let mut registrations: Vec<TypeRegistration> = [
        NOTE_DEFINITION,
        TASK_DEFINITION,
        PROJECT_DEFINITION,
        TAG_DEFINITION,
        PERSON_DEFINITION,
        IMAGE_DEFINITION,
        TIME_ENTRY_DEFINITION,
        GAME_DEFINITION,
        BOOK_DEFINITION,
    ]
    .into_iter()
    .map(registration_from_literal)
    .collect::<Result<_, String>>()?;
    // KOS-297: `scheduledAt`, `dueAt`, and `recurrence.endDate` are
    // day-granularity fields — writers store "YYYY-MM-DD". Version 1.0.0
    // declared `format: "date-time"`, a contract nothing enforced and writers
    // never honored; 1.1.0 declares `format: "date"` and the validator
    // enforces it on write.
    const DAY_FIELDS: &[&[&str]] = &[&["scheduledAt"], &["dueAt"], &["recurrence", "endDate"]];
    registrations.push(evolved_registration(
        TASK_DEFINITION,
        DAY_CONTRACT_VERSION,
        DAY_FIELDS,
    )?);
    registrations.push(evolved_registration(
        PROJECT_DEFINITION,
        DAY_CONTRACT_VERSION,
        &DAY_FIELDS[..2],
    )?);
    Ok(registrations)
}

/// Newest registered version of a canonical type. `None` for unknown types.
pub fn current_canonical_version(type_id: &str) -> Result<Option<String>, String> {
    Ok(canonical_type_registrations()?
        .iter()
        .filter(|r| r.type_id == type_id)
        .filter_map(|r| semver::Version::parse(&r.version).ok())
        .max()
        .map(|v| v.to_string()))
}

/// Whether `(type_id, version)` is a live canonical registration. Readers of
/// stored objects must accept every registered version — objects keep the
/// version they were written with.
pub fn is_canonical_version(type_id: &str, version: &str) -> bool {
    canonical_type_registrations()
        .map(|registrations| {
            registrations
                .iter()
                .any(|r| r.type_id == type_id && r.version == version)
        })
        .unwrap_or(false)
}

