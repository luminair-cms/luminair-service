# Config Schema Documentation

## Overview

Luminair uses a schema-driven approach to define document types (entities) for the CMS. Schemas are defined in JSON files located in the `config/schema/` directory. These schemas describe the structure of collections and single-type documents, including their attributes, relationships, and options.

## Schema File Structure

Each schema file is a JSON object with the following structure:

```json
{
  "type": "collection" | "singleType",
  "info": {
    "title": "Display Title",
    "singularName": "singular-name",
    "pluralName": "plural-name",
    "description": "Optional description"
  },
  "options": {
    "draftAndPublish": true | false,
    "localizations": ["en", "ro", "ru"]
  },
  "attributes": {
    "attributeName": {
      // Field or relation definition
    }
  }
}
```

### Document Type

- `"collection"`: Defines a collection of documents (e.g., multiple brands, partners)
- `"singleType"`: Defines a single document instance (not currently used in existing schemas)

### Info Section

- `title`: Human-readable title for the document type
- `singularName`: Singular form of the API identifier (used for single-type documents)
- `pluralName`: Plural form of the API identifier (used for collections)
- `description`: Optional description of the document type

### Options Section

- `draftAndPublish`: Boolean indicating if the document type supports draft/publish workflow
- `localizations`: Array of supported localization identifiers (e.g., `["en", "ro"]`)

### Attributes Section

Attributes define the fields and relationships for the document type. Each attribute can be either a field or a relation.

#### Field Attributes

```json
"attributeName": {
  "type": "fieldType",
  "unique": true | false,
  "required": true | false,
  "constraints": [
    // Field-specific constraints
  ]
}
```

Supported field types:
- `"text"`: Text string
- `"uid"`: Unique identifier
- `"localizedText"`: Localized text (requires localization support)
- `"integer"`: Integer with size specification
- `"decimal"`: Decimal with precision and scale

Field properties:
- `unique`: Whether the field value must be unique across all documents
- `required`: Whether the field is mandatory
- `constraints`: Array of validation constraints (e.g., length limits, patterns)

#### Field Constraints

Constraints provide additional validation rules for field values. The following constraints are supported:

- `pattern`: Regular expression pattern for text validation (applicable to `text` and `uid` fields)
- `minimalLength`: Minimum character length (applicable to text fields)
- `maximalLength`: Maximum character length (applicable to text fields)
- `minimalIntegerValue`: Minimum integer value (applicable to `integer` fields)
- `maximalIntegerValue`: Maximum integer value (applicable to `integer` fields)

**Examples from existing schemas:**

1. **UID with length constraints** (from `brands.json`):
```json
"uid": {
  "type": "uid",
  "unique": true,
  "required": true,
  "constraints": [
    { "minimalLength": 4 },
    { "maximalLength": 10 }
  ]
}
```

2. **Text with pattern validation** (from `partners.json`):
```json
"idno": {
  "type": "text",
  "unique": true,
  "required": true,
  "constraints": [
    { "pattern": "^[0-9]{13}$" }
  ]
}
```

3. **Decimal with precision and scale** (from `points-of-sale.json`):
```json
"latitude": {
  "type": {
    "decimal": {
      "precision": 10,
      "scale": 8
    }
  },
  "required": true
}
```

The loading logic validates that constraints are applicable to their field types. For example, `minimalLength` can only be used with text-based fields (`text`, `uid`, `localizedText`), while `minimalIntegerValue` can only be used with `integer` fields.

#### Relation Attributes

```json
"attributeName": {
  "relation": "relationType",
  "target": "targetDocumentType"
}
```

Relation types:
- `"hasOne"`: One-to-one relationship (this document has one related document)
- `"hasMany"`: One-to-many relationship (this document has many related documents)
- `"belongsToOne"`: Belongs to one (inverse of hasOne)
- `"belongsToMany"`: Belongs to many (inverse of hasMany)

## Loading Logic

The schema loading process is handled by the `load()` function in `common/src/infrastructure/documents.rs`:

1. Reads all `.json` files from the configured schema directory (`schema_config_path`)
2. Parses each JSON file into a `DocumentRecord`
3. Converts records to `DocumentType` instances with validation
4. Builds a registry mapping API IDs to document types
5. Stores the registry in a static `OnceLock` for runtime access

The registry provides:
- Iteration over all document types
- Lookup by document type ID
- Lookup by API ID (singular name for single types, plural name for collections)

## Configuration

The schema directory path is configured in `config/default.yaml`:

```yaml
schema_config_path: ./config/schema
```

## Validation and Constraints

The loading process validates:
- JSON syntax and structure
- Field type compatibility with constraints
- Relationship target existence
- Localization identifier validity
- Unique and required field rules

Invalid schemas will cause the application to fail during startup with detailed error messages.