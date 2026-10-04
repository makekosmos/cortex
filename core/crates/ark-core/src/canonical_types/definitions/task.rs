pub(crate) const TASK_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.task",
  "name": "Задача",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["task_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "status": {
        "type": "string",
        "enum": ["inbox", "todo", "inProgress", "done", "canceled"],
        "default": "inbox"
      },
      "priority": {
        "type": "string",
        "enum": ["none", "low", "medium", "high", "urgent"],
        "default": "none"
      },
      "scheduledAt": {
        "type": ["string", "null"],
        "default": null,
        "format": "date-time"
      },
      "dueAt": {
        "type": ["string", "null"],
        "default": null,
        "format": "date-time"
      },
      "reminderAt": {
        "type": ["string", "null"],
        "default": null,
        "format": "date-time"
      },
      "completedAt": {
        "type": ["string", "null"],
        "default": null,
        "format": "date-time"
      },
      "canceledAt": {
        "type": ["string", "null"],
        "default": null,
        "format": "date-time"
      },
      "recurrence": {
        "type": ["object", "null"],
        "default": null,
        "additionalProperties": false,
        "properties": {
          "frequency": {
            "type": "string",
            "enum": ["daily", "weekly", "monthly", "yearly"]
          },
          "interval": {
            "type": "integer",
            "minimum": 1,
            "default": 1
          },
          "recurrenceType": {
            "type": "string",
            "enum": ["fixed", "afterCompletion"]
          },
          "daysOfWeek": {
            "type": ["array", "null"],
            "items": {
              "type": "integer",
              "minimum": 0,
              "maximum": 6
            },
            "uniqueItems": true,
            "default": null
          },
          "endDate": {
            "type": ["string", "null"],
            "format": "date-time",
            "default": null
          }
        },
        "required": ["frequency", "interval", "recurrenceType", "daysOfWeek", "endDate"]
      },
      "checklist": {
        "type": "array",
        "items": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "id": {
              "type": "string",
              "minLength": 1
            },
            "title": {
              "type": "string"
            },
            "isCompleted": {
              "type": "boolean",
              "default": false
            }
          },
          "required": ["id", "title", "isCompleted"]
        },
        "default": []
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": [
      "status",
      "priority",
      "scheduledAt",
      "dueAt",
      "reminderAt",
      "completedAt",
      "canceledAt",
      "recurrence",
      "checklist",
      "extensions"
    ]
  },
  "uiSchema": {
    "collectionName": "Задачи",
    "featuredFields": ["status", "priority"],
    "visibleFields": [
      "status",
      "priority",
      "scheduledAt",
      "dueAt",
      "reminderAt",
      "completedAt",
      "canceledAt",
      "recurrence",
      "checklist"
    ],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": [
      "status",
      "priority",
      "scheduledAt",
      "dueAt",
      "reminderAt",
      "completedAt",
      "canceledAt",
      "recurrence",
      "checklist"
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
      "type": "project",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.project",
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
      "type": "related",
      "direction": "outbound",
      "targetTypeId": null,
      "min": 0,
      "max": null
    },
    {
      "type": "source-note",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.note",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "7e4d010591eb8356224b390725f5eb8ac4347c005d66550d389e9acd08f86a4d"
}"###;
