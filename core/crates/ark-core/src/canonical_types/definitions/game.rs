pub(crate) const GAME_DEFINITION: &str = r###"{
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
