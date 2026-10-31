// 0.2.0 retargets task relations to the canonical core project/tag types.
pub(crate) const CORE_TASK_DEFINITION: &str = r###"{
  "typeId": "core.task",
  "name": "Задача",
  "version": "0.2.0",
  "ownerKind": "core",
  "ownerId": "com.kosmos.core",
  "status": "active",
  "baseTypeId": null,
  "createdAt": "1970-01-01T00:00:00.000Z",
  "aliases": [],
  "schema": {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "state": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "kind": {
            "type": "string",
            "enum": [
              "open",
              "inProgress",
              "done",
              "canceled"
            ],
            "default": "open"
          },
          "at": {
            "type": [
              "string",
              "null"
            ],
            "format": "date-time",
            "default": null
          }
        },
        "required": [
          "kind",
          "at"
        ]
      },
      "planning": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "kind": {
            "type": "string",
            "enum": [
              "inbox",
              "anytime",
              "someday",
              "today",
              "date"
            ],
            "default": "inbox"
          },
          "period": {
            "type": "string",
            "enum": [
              "any",
              "evening"
            ],
            "default": "any"
          },
          "date": {
            "type": [
              "string",
              "null"
            ],
            "format": "date",
            "default": null
          }
        },
        "required": [
          "kind",
          "period",
          "date"
        ]
      },
      "deadline": {
        "type": [
          "string",
          "null"
        ],
        "format": "date",
        "default": null
      },
      "reminders": {
        "type": "array",
        "default": [],
        "items": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "kind": {
              "type": "string",
              "enum": [
                "floating",
                "local",
                "instant"
              ]
            },
            "at": {
              "type": "string"
            },
            "timeZone": {
              "type": [
                "string",
                "null"
              ],
              "default": null
            }
          },
          "required": [
            "kind",
            "at",
            "timeZone"
          ]
        }
      },
      "recurrence": {
        "type": [
          "object",
          "null"
        ],
        "default": null,
        "additionalProperties": false,
        "properties": {
          "kind": {
            "type": "string",
            "enum": [
              "calendar",
              "afterCompletion"
            ]
          },
          "frequency": {
            "type": "string",
            "enum": [
              "daily",
              "weekly",
              "monthly",
              "yearly"
            ]
          },
          "interval": {
            "type": "integer",
            "minimum": 1,
            "default": 1
          },
          "daysOfWeek": {
            "type": [
              "array",
              "null"
            ],
            "items": {
              "type": "integer",
              "minimum": 1,
              "maximum": 7
            },
            "uniqueItems": true,
            "default": null
          },
          "timeZone": {
            "type": [
              "string",
              "null"
            ],
            "default": null
          },
          "end": {
            "type": [
              "object",
              "null"
            ],
            "default": null,
            "additionalProperties": false,
            "properties": {
              "kind": {
                "type": "string",
                "enum": [
                  "until",
                  "count"
                ]
              },
              "date": {
                "type": [
                  "string",
                  "null"
                ],
                "format": "date",
                "default": null
              },
              "count": {
                "type": [
                  "integer",
                  "null"
                ],
                "minimum": 1,
                "default": null
              }
            },
            "required": [
              "kind",
              "date",
              "count"
            ]
          }
        },
        "required": [
          "kind",
          "frequency",
          "interval",
          "daysOfWeek",
          "timeZone",
          "end"
        ]
      },
      "priority": {
        "type": "string",
        "enum": [
          "none",
          "low",
          "medium",
          "high",
          "urgent"
        ],
        "default": "none"
      },
      "checklist": {
        "type": "array",
        "default": [],
        "items": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "id": {
              "type": "string",
              "minLength": 1
            },
            "text": {
              "type": "string"
            },
            "done": {
              "type": "boolean",
              "default": false
            },
            "rank": {
              "type": "string",
              "minLength": 1
            }
          },
          "required": [
            "id",
            "text",
            "done",
            "rank"
          ]
        }
      },
      "extensions": {
        "type": "object",
        "default": {},
        "additionalProperties": true
      }
    },
    "required": [
      "state",
      "planning",
      "deadline",
      "reminders",
      "recurrence",
      "priority",
      "checklist",
      "extensions"
    ]
  },
  "uiSchema": {
    "collectionName": "Задачи",
    "featuredFields": [
      "state",
      "planning",
      "priority"
    ],
    "visibleFields": [
      "state",
      "planning",
      "deadline",
      "reminders",
      "recurrence",
      "priority",
      "checklist"
    ],
    "hiddenFields": [
      "extensions"
    ],
    "readOnlyFields": [],
    "fieldOrder": [
      "state",
      "planning",
      "deadline",
      "reminders",
      "recurrence",
      "priority",
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
    "allowedMarks": [
      "bold",
      "italic",
      "strike",
      "code",
      "link",
      "underline"
    ],
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
          "type": [
            "string",
            "null"
          ],
          "default": null
        }
      },
      "link": {
        "href": {
          "type": "string"
        },
        "target": {
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "rel": {
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "class": {
          "type": [
            "string",
            "null"
          ],
          "default": null
        }
      },
      "image": {
        "src": {
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "alt": {
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "title": {
          "type": [
            "string",
            "null"
          ],
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
      "targetTypeId": "core.project",
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
    "include": [
      "title",
      "content",
      "props",
      "links",
      "createdAt",
      "updatedAt",
      "deletedAt"
    ],
    "exclude": [
      "objectLocalState",
      "secrets"
    ]
  },
  "schemaHash": "7deaeac7698789a497fdb17f2e005c3b8f6c2d046576685173fe8b6467fbefef"
}"###;
