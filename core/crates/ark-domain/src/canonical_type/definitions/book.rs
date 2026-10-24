pub(crate) const BOOK_DEFINITION: &str = r###"{
  "typeId": "core.book",
  "name": "Книга",
  "version": "0.1.0",
  "ownerKind": "core",
  "ownerId": "core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": [],
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
      "targetTypeId": "core.image",
      "min": 0,
      "max": 1
    },
    {
      "type": "tag",
      "direction": "outbound",
      "targetTypeId": "core.tag",
      "min": 0,
      "max": null
    },
    {
      "type": "author-person",
      "direction": "outbound",
      "targetTypeId": "core.person",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "7217e0ef3d1b48f250a904891593a4bdfb41d8f3718da4eaa542a81a5d5a40ed"
}"###;
