# Backend part of the Luminair CMS

This is a work in progress at a very early stage.

Currently, the main goal of project - learning or Rust and investigate its possibilities.

Project is based on Strapi ideas but focused on Speed and Reliability.

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

## Documents model

### Initial document with

id, document_type, info, options, attributes

### Persisted document with

- document_ref (&'static) and persistence details
- main table details, needed for migration and mapping
- localization table details (Optional), needed for migration and mapping
- hashmap of fields, each field has needed attribute and persistence details
- hashmap of relations, each relation has ref to PersistedDocument and persistence details

### Associations

- One way: collection A has one row of collection B
- One to one: collection A has one row of collection B, and collection B has one row of collection A
- One to many: collection A has many rows of collection B
- Many to one: collection B has many rows of collection A
- Many to many: collection A has many rows of collection B, and collection B has many rows of collection A
- Many way: collection A has many rows of collection B

### mappedBy and inversedBy

- mappedBy and inversedBy are used to define the relationship between the owning and inverse sides of a bidirectional association.
- mappedBy: This attribute is used on the inverse side of the relationship. It specifies the property name on the owning entity that holds the association. In a Many-to-Many relationship, the inverse side does not manage the foreign keys in the join table directly. Instead, it relies on the owning side to define and manage the relationship.
- inversedBy: This attribute is used on the owning side of the relationship. It specifies the property name on the inverse entity that holds the association. The owning side is responsible for managing the join table and the foreign keys that establish the Many-to-Many relationship.

### The following general rules apply:

- Relationships may be bidirectional or unidirectional.
- A bidirectional relationship has both an owning side and an inverse side
- A unidirectional relationship only has an owning side.
- Doctrine will only check the owning side of an association for changes.

## Bidirectional Associations

### The following rules apply to bidirectional associations:

- The inverse side has to have the mappedBy attribute of the OneToOne, OneToMany, or ManyToMany mapping declaration. The mappedBy attribute contains the name of the association-field on the owning side.
- The owning side has to have the inversedBy attribute of the OneToOne, ManyToOne, or ManyToMany mapping declaration. The inversedBy attribute contains the name of the association-field on the inverse-side.
- ManyToOne is always the owning side of a bidirectional association.
- OneToMany is always the inverse side of a bidirectional association.
- The owning side of a OneToOne association is the entity with the table containing the foreign key.
- You can pick the owning side of a many-to-many association yourself.

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
      "document_id": 2,
      "Name": "BMK Paris Bamako",
      "Description": {
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
      "document_id": 4,
      "documentId": "znrlzntu9ei5onjvwfaalu2v",
      "Name": "Biscotte Restaurant",
      "Description": [
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
    "document_id": 6,
    "Name": "Biscotte Restaurant",
    "Description": [
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
