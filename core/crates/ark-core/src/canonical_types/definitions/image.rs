pub(crate) const IMAGE_DEFINITION: &str = r###"{
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
