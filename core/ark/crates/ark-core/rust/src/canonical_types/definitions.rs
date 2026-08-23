//! Immutable first-party canonical type definitions.
//!
//! This module is pure domain data: it does not perform SQL, migration, validation,
//! event, or registry writes. The complete JSON fixtures below are the frozen
//! authority; the existing type-registry hash implementation remains authoritative.

use crate::type_registry::{canonical_schema_hash, AliasRecord, TypeRegistration};
use serde_json::Value;

const NOTE_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.note",
  "name": "Заметка",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["note_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "description": {
        "type": ["string", "null"],
        "default": null
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": ["description", "extensions"]
  },
  "uiSchema": {
    "collectionName": "Заметки",
    "featuredFields": [],
    "visibleFields": ["description"],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": ["description"],
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
      "type": "related",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.note",
      "min": 0,
      "max": null
    },
    {
      "type": "tag",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.tag",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "8dae6b0a41de279ab6176bfc2115ec88c08b40551bb5ed566a3773b0cac2be53"
}"###;
const TASK_DEFINITION: &str = r###"{
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
const PROJECT_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.project",
  "name": "Проект",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["project_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "status": {
        "type": "string",
        "enum": ["active", "someday", "completed"],
        "default": "active"
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
      "color": {
        "type": ["string", "null"],
        "default": null
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": ["status", "scheduledAt", "dueAt", "color", "extensions"]
  },
  "uiSchema": {
    "collectionName": "Проекты",
    "featuredFields": ["status"],
    "visibleFields": ["status", "scheduledAt", "dueAt", "color"],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": ["status", "scheduledAt", "dueAt", "color"],
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
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "55ea05493991deda6093dc86c73e30d05f5db3b4b1d42c9b675d6b176a633bac"
}"###;
const TAG_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.tag",
  "name": "Тег",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["tag_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "color": {
        "type": ["string", "null"],
        "default": null
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": ["color", "extensions"]
  },
  "uiSchema": {
    "collectionName": "Теги",
    "featuredFields": [],
    "visibleFields": ["color"],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": ["color"],
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
      "type": "related",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.tag",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "de364bb69edcad14f474b018e65c2ccc8be53426133d5107ac9cbd9492d5b18f"
}"###;
const PERSON_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.person",
  "name": "Человек",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["person_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "firstName": {
        "type": ["string", "null"],
        "default": null
      },
      "lastName": {
        "type": ["string", "null"],
        "default": null
      },
      "patronymic": {
        "type": ["string", "null"],
        "default": null
      },
      "birthDate": {
        "type": ["string", "null"],
        "default": null,
        "format": "date"
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": ["firstName", "lastName", "patronymic", "birthDate", "extensions"]
  },
  "uiSchema": {
    "collectionName": "Люди",
    "featuredFields": ["firstName", "lastName"],
    "visibleFields": ["firstName", "lastName", "patronymic", "birthDate"],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": ["firstName", "lastName", "patronymic", "birthDate"],
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
      "type": "photo",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.image",
      "min": 0,
      "max": 1
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "664f5f1858db443e53c0d304c29822a00fa862c48e361fca455350adc64372c3"
}"###;
const IMAGE_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.image",
  "name": "Изображение",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["image_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "fileName": {
        "type": ["string", "null"],
        "default": null
      },
      "mimeType": {
        "type": ["string", "null"],
        "default": null
      },
      "sizeBytes": {
        "type": ["integer", "null"],
        "default": null,
        "minimum": 0
      },
      "width": {
        "type": ["integer", "null"],
        "default": null,
        "minimum": 0
      },
      "height": {
        "type": ["integer", "null"],
        "default": null,
        "minimum": 0
      },
      "resolution": {
        "type": ["string", "null"],
        "default": null
      },
      "altText": {
        "type": "string",
        "default": ""
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": [
      "fileName",
      "mimeType",
      "sizeBytes",
      "width",
      "height",
      "resolution",
      "altText",
      "extensions"
    ]
  },
  "uiSchema": {
    "collectionName": "Изображения",
    "featuredFields": ["fileName", "resolution"],
    "visibleFields": [
      "fileName",
      "mimeType",
      "sizeBytes",
      "width",
      "height",
      "resolution",
      "altText"
    ],
    "hiddenFields": ["extensions"],
    "readOnlyFields": ["mimeType", "sizeBytes", "width", "height", "resolution"],
    "fieldOrder": ["fileName", "mimeType", "sizeBytes", "width", "height", "resolution", "altText"],
    "headerLayout": "inline",
    "defaultLayout": "page",
    "defaultTemplateId": null
  },
  "contentContract": {
    "mediaType": "application/json",
    "shape": "object"
  },
  "relations": [],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "83e9bc320b20236b48b29294c084ca2adaecb83ddd8702217ee220641ca193ff"
}"###;
const TIME_ENTRY_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.time-entry",
  "name": "Запись времени",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["time_entry_obj"],
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
      "targetTypeId": "com.kosmos.task",
      "min": 0,
      "max": 1
    },
    {
      "type": "tag",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.tag",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "e455f5ef3e0b60bcd5097488c2e8ccdbf8c58ace92d02a22d94f8496ff5ed4b5"
}"###;
const GAME_DEFINITION: &str = r###"{
  "typeId": "com.kosmos.game",
  "name": "Игра",
  "version": "1.0.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": ["game_obj"],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "playStatus": {
        "type": ["string", "null"],
        "enum": ["notStarted", "inProgress", "completed", "abandoned", null],
        "default": null
      },
      "userRating": {
        "type": ["number", "null"],
        "default": null,
        "minimum": 0,
        "maximum": 10
      },
      "genres": {
        "type": "array",
        "items": {
          "type": "string"
        },
        "default": []
      },
      "platforms": {
        "type": "array",
        "items": {
          "type": "string"
        },
        "default": []
      },
      "released": {
        "type": ["string", "null"],
        "default": null,
        "format": "date"
      },
      "description": {
        "type": ["string", "null"],
        "default": null
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": [
      "playStatus",
      "userRating",
      "genres",
      "platforms",
      "released",
      "description",
      "extensions"
    ]
  },
  "uiSchema": {
    "collectionName": "Игры",
    "featuredFields": ["playStatus", "genres"],
    "visibleFields": ["playStatus", "userRating", "genres", "platforms", "released", "description"],
    "hiddenFields": ["extensions"],
    "readOnlyFields": [],
    "fieldOrder": ["playStatus", "userRating", "genres", "platforms", "released", "description"],
    "headerLayout": "inline",
    "defaultLayout": "page",
    "defaultTemplateId": null
  },
  "contentContract": {
    "mediaType": "application/json",
    "shape": "object"
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
      "type": "background-image",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.image",
      "min": 0,
      "max": 1
    },
    {
      "type": "note",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.note",
      "min": 0,
      "max": null
    },
    {
      "type": "task",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.task",
      "min": 0,
      "max": null
    },
    {
      "type": "tag",
      "direction": "outbound",
      "targetTypeId": "com.kosmos.tag",
      "min": 0,
      "max": null
    }
  ],
  "syncPolicy": {
    "scope": "shared",
    "include": ["title", "content", "props", "links", "createdAt", "updatedAt", "deletedAt"],
    "exclude": ["objectLocalState", "secrets"]
  },
  "schemaHash": "de41b108ef21958b901851a92561dfb046cffcedf29e6cb7c355d97396f1ba99"
}"###;
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

/// Return the nine frozen, ordered first-party registrations.
pub fn canonical_type_registrations() -> Result<Vec<TypeRegistration>, String> {
    [
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
    .collect()
}

fn registration_from_literal(raw: &str) -> Result<TypeRegistration, String> {
    let definition: Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    let object = definition
        .as_object()
        .ok_or_else(|| "canonical definition must be a JSON object".to_owned())?;
    let string = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("canonical definition field {field} must be a string"))
    };
    let schema = object.get("schema").ok_or("missing schema")?;
    let ui_schema = object.get("uiSchema").ok_or("missing uiSchema")?;
    let content_contract = object
        .get("contentContract")
        .ok_or("missing contentContract")?;
    let relations = object.get("relations").ok_or("missing relations")?;
    let sync_policy = object.get("syncPolicy").ok_or("missing syncPolicy")?;
    let expected_hash = string("schemaHash")?;
    let actual_hash =
        canonical_schema_hash(schema, ui_schema, content_contract, relations, sync_policy)?;
    if actual_hash != expected_hash {
        return Err(format!(
            "frozen canonical hash mismatch for {}: expected {expected_hash}, got {actual_hash}",
            string("typeId")?
        ));
    }
    let aliases = object
        .get("aliases")
        .and_then(Value::as_array)
        .ok_or("canonical definition aliases must be an array")?
        .iter()
        .map(|alias| {
            let alias = alias
                .as_str()
                .ok_or("canonical definition alias must be a string")?;
            Ok(AliasRecord {
                alias: alias.to_owned(),
                canonical_type_id: string("typeId")?,
                created_at: string("createdAt")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TypeRegistration {
        type_id: string("typeId")?,
        name: string("name")?,
        schema_json: serde_json::to_string(schema).map_err(|error| error.to_string())?,
        ui_schema_json: serde_json::to_string(ui_schema).map_err(|error| error.to_string())?,
        content_contract_json: serde_json::to_string(content_contract)
            .map_err(|error| error.to_string())?,
        relations_json: serde_json::to_string(relations).map_err(|error| error.to_string())?,
        sync_policy_json: serde_json::to_string(sync_policy).map_err(|error| error.to_string())?,
        version: string("version")?,
        schema_hash: expected_hash,
        owner_kind: string("ownerKind")?,
        owner_id: object
            .get("ownerId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        status: string("status")?,
        base_type_id: object
            .get("baseTypeId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        aliases,
        created_at: string("createdAt")?,
    })
}
