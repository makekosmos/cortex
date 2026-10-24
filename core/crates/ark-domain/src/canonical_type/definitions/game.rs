pub(crate) const GAME_DEFINITION: &str = r###"{
  "typeId": "core.game",
  "name": "Игра",
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
      "targetTypeId": "core.image",
      "min": 0,
      "max": 1
    },
    {
      "type": "background-image",
      "direction": "outbound",
      "targetTypeId": "core.image",
      "min": 0,
      "max": 1
    },
    {
      "type": "note",
      "direction": "outbound",
      "targetTypeId": "core.note",
      "min": 0,
      "max": null
    },
    {
      "type": "task",
      "direction": "outbound",
      "targetTypeId": "core.task",
      "min": 0,
      "max": null
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
  "schemaHash": "37c858f0ba362a4df247f50b4955ae9b92961671664b66314f019ac06e8f420c"
}"###;
