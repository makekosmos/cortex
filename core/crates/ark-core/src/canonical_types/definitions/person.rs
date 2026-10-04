pub(crate) const PERSON_DEFINITION: &str = r###"{
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
