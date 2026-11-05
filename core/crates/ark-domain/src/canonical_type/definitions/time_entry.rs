pub(crate) const TIME_ENTRY_DEFINITION: &str = r###"{
  "typeId": "core.time-entry",
  "name": "Запись времени",
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
      "startedAt": {
        "type": "string",
        "format": "date-time"
      },
      "endedAt": {
        "type": ["string", "null"],
        "default": null,
        "format": "date-time"
      },
      "billable": {
        "type": "boolean",
        "default": false
      },
      "source": {
        "type": "string",
        "enum": ["manual", "pomodoro", "imported"],
        "default": "manual"
      },
      "taskTitle": {
        "type": ["string", "null"],
        "default": null
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": ["startedAt", "endedAt", "billable", "source", "taskTitle", "extensions"]
  },
  "uiSchema": {
    "collectionName": "Записи времени",
    "featuredFields": ["startedAt", "endedAt"],
    "visibleFields": ["startedAt", "endedAt", "billable", "source", "taskTitle"],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": ["startedAt", "endedAt", "billable", "source", "taskTitle"],
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
      "type": "for-task",
      "direction": "outbound",
      "targetTypeId": "core.task",
      "min": 0,
      "max": 1
    },
    {
      "type": "tag",
      "direction": "outbound",
      "targetTypeId": "core.tag",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "84c8bebd6e44a635879c32a57f6a1dc3e69a46bce08f43a410efffe5140735e9"
}"###;
