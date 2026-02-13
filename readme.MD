Luminair is a Schema Driven CMS platform

# Backend part of the Luminair CMS

This is a work in progress at a very early stage.

Currently, the main goal of project - learning or Rust and investigate its possibilities.

Project is based on Strapi ideas but focused on Speed and Reliability.

### Crates:

*Common*

Contains Documents schema and database infrastructure

*Migration*

Migration cli, uses Schema Registry for tables creation, deletion etc. Need DB pribilegies for DDL

*Service*

Microservice, uses Schema Registry, provide dynamic api for Schama Metadata and Documents manipulation. Need privileges only for DML

## Main principles

### Domain driven design and Hexagonal architecture where it is appropriate.

see resources:

- https://berk.es/2022/09/28/value-objects-in-rust/
- https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust
- https://www.howtocodeit.com/guides/the-definitive-guide-to-rust-error-handling
- https://www.howtocodeit.com/guides/ultimate-guide-rust-newtypes
- https://bitfieldconsulting.com/posts/best-rust-books

### Microservices architecture, separated parts

- common crate for common domain model and logic
- migration create for a separate migration cli tool that will be started before the main service with DDL privileges
- service for microservice that provides rest api

* Also, will be separate UI repository (probably based on React) and a separate UI tool for schema generation.

## Rust design patterns and idioms

https://rust-unofficial.github.io/patterns/patterns/creational/builder.html

https://dsar.rantai.dev/docs/dsar/

https://www.sea-ql.org/blog/2026-01-12-sea-orm-2.0/

## Documents model

### Initial document with

id, document_type, info, options, attributes
id of document is taken from file-name and used in normalized form for table generation

For document_type_id in rest api used plural name in case of collection and singular name in case of singleton

### Persisted document with

For table-name used singular name. 
Fields of table:
| field         | description |
|---------------|-------------|
| id            | id of database row, serial, rimary key |
| document_id   | id of document instance, UUID          |
| field 1 .. field N | fields of document               |
| published_at  | timestamp of publication, in case of draft_and_publish enabled |
| published_by_id | user_id who performed publish
| revision      |
| created_at    |
| updated_at    |
| created_by_id | integer
| updated_by_id | integer
| version       | integer

## How `PublicationState` and `AuditTrail` Work Together**

### **Two Separate Concerns**

1. **`PublicationState`** (in `DocumentContent`) - **Domain Logic**
   - Tracks the **content's publication workflow** (Draft vs Published)
   - Stores the `revision` number tied to **this specific publication state**
   - Includes `published_at` as part of the **publication record** (when this state was achieved)

2. **`AuditTrail`** (in `DocumentInstance`) - **Infrastructure/System Metadata**
   - Tracks **WHO did WHAT WHEN** across all operations
   - Stores the `version` number for **overall document lifecycle tracking**
   - Includes `published_at` as part of **system audit records**

---

### **Relationship Between `revision` and `version`**

| Aspect | `revision` (in PublicationState) | `version` (in AuditTrail) |
|--------|----------------------------------|--------------------------|
| **Purpose** | Tracks publication cycle iteration | Tracks overall change history |
| **Scope** | Only for published versions | Every state change (edit, publish, unpublish) |
| **When Updated** | When transitioning Draft → Published | Every operation that modifies the document |
| **Example** | Publishing creates revision 1, 2, 3... | Increments with edits AND publishes |

**Concrete Example:**
```
1. Create document → version: 1, state: Draft { revision: 1 }
2. Edit content  → version: 2, state: Draft { revision: 1 }  (revision stays same)
3. Edit again    → version: 3, state: Draft { revision: 1 }  (revision stays same)
4. Publish       → version: 4, state: Published { revision: 1, published_at: T1 }
5. Edit          → version: 5, state: Draft { revision: 2 }   (revision increments!)
6. Publish       → version: 6, state: Published { revision: 2, published_at: T2 }
```

---

### **Why `published_at` Exists in BOTH Structures**

| Where | Why | Concern |
|-------|-----|---------|
| **`PublicationState::Published`** | Records **when this publication revision was created** | **Domain**: Part of the publication state definition |
| **`AuditTrail.published_at`** | Records **when the last publish action occurred** | **Infrastructure**: System audit trail for compliance |

**Key Difference:**
- If you unpublish then republish, `AuditTrail.published_at` updates to the latest publish time
- But `PublicationState.Published { published_at }` would be from a **different revision**, each with its own timestamp

### Associations

- HasOne: collection A has one row of collection B
- HasMany: collection A has many rows of collection B
- BelongsToOne: collection B belongs to one row of collection A
- BelongsToMany: collection B belongs to many rows of Collection B

*The following general rules apply*

- HasOne & HasMany are attributes of owning side, collection that own this relation
- BelongsToOne & BelongsToMany are attributes of inverse side

- Relationships may be bidirectional or unidirectional.
- A bidirectional relationship has both an owning side and an inverse side
- A unidirectional relationship only has an owning side.

*Persistence of associations*

For owning table-name used singular name underscore-concatenated with attribute-name underscore-concatenated with "relation" suffix. 
Fields of table:
| field        | description |
|--------------|-------------|
| id           | id of database row, serial, rimary key |
| owning-table-name-id | owning table-name underscore-concatenated with "id" suffix |
| inverse-table-name-id | inverse table-name underscore-concatenated with "id" suffix |

## Rest API

Rest API is modeled after same in Strapi. See: https://docs.strapi.io/cms/api/rest

### Plural API ID vs Singular API ID

:singularApiId refers to the value of the "API ID (Singular)" field of the content-type,
and :pluralApiId refers to the value of the "API ID (Plural)" field of the content-type.
These values are defined when creating a content-type in the Content-Type Builder, and can be found while editing a content-type in the admin panel (see User Guide). For instance, by default, for an "Article" content-type:

    :singularApiId will be article

    :pluralApiId will be articles

### Example of plural api

| Method | URL                          | Description               |
| ------ | ---------------------------- | ------------------------- |
| GET    | /api/restaurants             | Get a list of restaurants |
| POST   | /api/restaurants             | Create a restaurant       |
| GET    | /api/restaurants/:documentId | Get a specific restaurant |
| DELETE | /api/restaurants/:documentId | Delete a restaurant       |
| PUT    | /api/restaurants/:documentId | Update a restaurant       |

### Example of singular api

| Method | URL           | Description                        |
| ------ | ------------- | ---------------------------------- |
| GET    | /api/homepage | Get the homepage content           |
| PUT    | /api/homepage | Update/create the homepage content |
| DELETE | /api/homepage | Delete the homepage content        |

### Example of response on /api/restaurants

```json
{
  "data": [
    {
      "id": 2,
      "documentId": "8d0ef031-2a9a-4ea3-980f-e2a7f4803e95",
      "name": "BMK Paris Bamako",
      "description": {
        "en": "Description on English language",
        "ro": "Descriptie la limba Romaina",
        "ru": "Описание на Русском языке"
      }
      "createdAt": "2024-03-06T13:42:05.098Z",
      "updatedAt": "2024-03-06T13:42:05.098Z",
      "publishedAt": "2024-03-06T13:42:05.103Z",
      }
    },
    {
      "id": 4,
      "documentId": "791620a6-1099-4a41-ad74-21c5a25ce9b2",
      "name": "Biscotte Restaurant",
      "description": [
        {
          "type": "paragraph",
          "children": [
            {
              "type": "text",
              "text": "Welcome to Biscotte restaurant! Restaurant Biscotte offers a cuisine based on fresh, quality products, often local, organic when possible, and always produced by passionate producers."
            }
          ]
        }
      ],
      "createdAt": "2024-03-06T13:43:30.172Z",
      "updatedAt": "2024-03-06T13:43:30.172Z",
      "publishedAt": "2024-03-06T13:43:30.175Z"
    }
  ],
  "meta": {
    "pagination": {
      "page": 1,
      "pageSize": 25,
      "pageCount": 1,
      "total": 2
    },
    "defaultLocale": "en"
  }
}
```

### Example of response on /api/restaurants/:documentId

```json
{
  "data": {
    "id": 6,
    "documentId": "791620a6-1099-4a41-ad74-21c5a25ce9b2"
    "name": "Biscotte Restaurant",
    "description": [
      {
        "type": "paragraph",
        "children": [
          {
            "type": "text",
            "text": "Welcome to Biscotte restaurant! Restaurant Biscotte offers a cuisine bassics, such as 4 Formaggi or Calzone, and our original creations such as Do Luigi or Nduja."
          }
        ]
      }
    ],
    "createdAt": "2024-02-27T10:19:04.953Z",
    "updatedAt": "2024-03-05T15:52:05.591Z",
    "publishedAt": "2024-03-05T15:52:05.600Z"
  },
  "meta": {
    "defaultLocale": "en"
  }
}
```
