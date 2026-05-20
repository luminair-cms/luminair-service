A **document identity table per collection type** is a strong design choice. I would support it.

But I would be careful with adding a stored `status = PUBLISHED | MODIFIED` field. It can be useful, but only if you define it as a **derived/cache field**, not as the source of truth.

The source of truth should still be the existence of draft/published content rows.

---

## Suggested model

For each collection, instead of only having:

```plain text
articles
```


you would have:

```plain text
article_documents
articles
article_relations...
```


Where:

```plain text
article_documents
-----------------
document_id uuid primary key
status text not null
created_at timestamp
updated_at timestamp
created_by_id text null
updated_by_id text null
```


And the content table becomes:

```plain text
articles
--------
id bigserial primary key
document_id uuid not null references article_documents(document_id)
publication_kind text not null -- DRAFT or PUBLISHED
revision int not null
published_at timestamp null
published_by_id text null
...
```


Then relation tables can safely reference the identity table:

```plain text
article_categories
------------------
owning_id bigint not null references articles(id)
inverse_document_id uuid not null references category_documents(document_id)
```


This gives you proper foreign keys again.

---

## Benefits

### 1. Relations become semantically correct

A relation points to the related document identity:

```plain text
inverse_document_id -> category_documents.document_id
```


not to one physical row version.

That means the relation says:

> This owner version is connected to this logical category.

At query time, the backend decides whether to load:

- the published category row
- the draft category row
- or nothing, depending on API context

That is exactly what draft-and-publish needs.

---

### 2. Foreign keys are restored

Without identity tables, `inverse_document_id` cannot safely reference the target content table because the content table may have multiple rows with the same `document_id`.

With identity tables:

```plain text
category_documents.document_id
```


is unique and stable.

So this works:

```sql
inverse_document_id uuid not null references category_documents(document_id)
```


That is a clean relational model.

---

### 3. Status becomes cheap to read

The editor UI often needs to show:

```plain text
Published
Modified
Draft
```


If you store status on the identity row, listing documents becomes simpler.

Example:

```sql
SELECT d.document_id, d.status, a.*
FROM article_documents d
JOIN articles a ON a.document_id = d.document_id
...
```


You do not always need to inspect both draft and published rows to determine status.

---

### 4. You get a natural place for document-level metadata

Some metadata belongs to the logical document, not to a specific draft/published row.

Good candidates:

```plain text
document_id
status
created_at
created_by_id
deleted_at
locked_by_id
workflow_state
scheduled_publish_at
```


Version-specific content remains in the content table.

---

## But do not limit status to only `PUBLISHED | MODIFIED`

I would not use only:

```plain text
PUBLISHED
MODIFIED
```


because you need at least one more state:

```plain text
DRAFT
```


A newly created document may have no published version yet.

So I recommend:

```plain text
DRAFT
PUBLISHED
MODIFIED
```


Meaning:

| Status | Meaning |
|---|---|
| `DRAFT` | Draft row exists, no published row exists |
| `PUBLISHED` | Published row exists, no draft row exists |
| `MODIFIED` | Published row exists and draft row exists |

Depending on your unpublish/delete model, you may later add:

```plain text
UNPUBLISHED
ARCHIVED
DELETED
SCHEDULED
```


But for now:

```plain text
DRAFT | PUBLISHED | MODIFIED
```


is the right minimum.

---

## Recommended identity table

For each collection:

```sql
CREATE TABLE article_documents (
    document_id uuid PRIMARY KEY,
    status text NOT NULL CHECK (status IN ('DRAFT', 'PUBLISHED', 'MODIFIED')),
    created_at timestamptz NOT NULL,
    created_by_id text NULL,
    updated_at timestamptz NOT NULL,
    updated_by_id text NULL
);
```


Then the content table:

```sql
CREATE TABLE articles (
    id bigserial PRIMARY KEY,
    document_id uuid NOT NULL REFERENCES article_documents(document_id) ON DELETE CASCADE,

    publication_state text NOT NULL CHECK (publication_state IN ('DRAFT', 'PUBLISHED')),
    revision integer NOT NULL,

    published_at timestamptz NULL,
    published_by_id text NULL,

    created_at timestamptz NOT NULL,
    created_by_id text NULL,
    updated_at timestamptz NOT NULL,
    updated_by_id text NULL,
    version integer NOT NULL,

    title text NULL,
    body text NULL
);
```


Add constraints:

```sql
CREATE UNIQUE INDEX articles_one_draft_per_document
    ON articles (document_id)
    WHERE publication_state = 'DRAFT';

CREATE UNIQUE INDEX articles_one_published_per_document
    ON articles (document_id)
    WHERE publication_state = 'PUBLISHED';
```


This is very important.

It guarantees:

```plain text
max one draft row
max one published row
```


per logical document.

---

## Recommended relation table

With per-collection identity tables, relation tables become:

```sql
CREATE TABLE article_categories (
    owning_id bigint NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    inverse_document_id uuid NOT NULL REFERENCES category_documents(document_id),

    PRIMARY KEY (owning_id, inverse_document_id)
);
```


Or with clearer naming:

```sql
CREATE TABLE article_categories (
    owning_id bigint NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    target_document_id uuid NOT NULL REFERENCES category_documents(document_id),

    PRIMARY KEY (owning_id, target_document_id)
);
```


I prefer:

```plain text
target_document_id
```


But if your domain already uses “inverse”, then `inverse_document_id` is fine.

---

## Should `status` be stored?

Yes, but with one warning:

> If you store `status`, you must keep it consistent transactionally.

Status is derivable from rows:

```plain text
has draft row?
has published row?
```


So this:

```plain text
article_documents.status
```


is denormalized data.

That is okay, but you must update it in the same transaction as every create/edit/publish/delete operation.

---

## Status transition rules

### Create new document

Create identity:

```plain text
article_documents:
document_id = X
status = DRAFT
```


Create content row:

```plain text
articles:
document_id = X
publication_state = DRAFT
```


---

### Publish draft-only document

Before:

```plain text
article_documents.status = DRAFT
articles: DRAFT row exists
```


After:

```plain text
article_documents.status = PUBLISHED
articles: PUBLISHED row exists
```


You may either:

1. Convert the draft row to published, or
2. Copy draft row to published row and delete draft row.

I recommend keeping only one active row per state, so after publish there should usually be no draft row.

---

### Edit published document

Before:

```plain text
article_documents.status = PUBLISHED
articles: PUBLISHED row exists
```


After:

```plain text
article_documents.status = MODIFIED
articles: PUBLISHED row exists
articles: DRAFT row exists
```


The draft row starts as a copy of the published row, then receives modifications.

---

### Edit already modified document

Before:

```plain text
article_documents.status = MODIFIED
articles: PUBLISHED row exists
articles: DRAFT row exists
```


After:

```plain text
article_documents.status = MODIFIED
articles: draft row updated
```


---

### Publish modified document

Before:

```plain text
article_documents.status = MODIFIED
articles: PUBLISHED row exists
articles: DRAFT row exists
```


After:

```plain text
article_documents.status = PUBLISHED
articles: PUBLISHED row updated/replaced from draft
articles: DRAFT row removed
```


---

### Delete draft-only document

Before:

```plain text
article_documents.status = DRAFT
articles: DRAFT row exists
```


After:

```plain text
delete article_documents row
cascade delete articles row
```


---

## Querying becomes much cleaner

### Public query

```sql
SELECT a.*, d.status
FROM article_documents d
JOIN articles a ON a.document_id = d.document_id
WHERE d.document_id = $1
  AND a.publication_state = 'PUBLISHED';
```


Only returns published content.

---

### Editor query

```sql
SELECT a.*, d.status
FROM article_documents d
JOIN articles a ON a.document_id = d.document_id
WHERE d.document_id = $1
ORDER BY
    CASE a.publication_state
        WHEN 'DRAFT' THEN 0
        WHEN 'PUBLISHED' THEN 1
    END
LIMIT 1;
```


Returns draft if present, otherwise published.

And the UI status comes directly from:

```plain text
article_documents.status
```


---

## Relation querying also becomes cleaner

For editor mode:

```sql
SELECT r.owning_id, c.*, cd.status
FROM article_categories r
JOIN category_documents cd
  ON cd.document_id = r.inverse_document_id
JOIN LATERAL (
    SELECT c.*
    FROM categories c
    WHERE c.document_id = cd.document_id
    ORDER BY
        CASE c.publication_state
            WHEN 'DRAFT' THEN 0
            WHEN 'PUBLISHED' THEN 1
        END
    LIMIT 1
) c ON true
WHERE r.owning_id = ANY($1);
```


For public mode:

```sql
SELECT r.owning_id, c.*, cd.status
FROM article_categories r
JOIN category_documents cd
  ON cd.document_id = r.inverse_document_id
JOIN categories c
  ON c.document_id = cd.document_id
 AND c.publication_state = 'PUBLISHED'
WHERE r.owning_id = ANY($1);
```


This lets the UI show related document status without extra queries.

---

## Per-collection identity table vs global identity table

You suggested:

> document identity table for each collection type

That is fine.

Example:

```plain text
article_documents
category_documents
author_documents
```


Pros:

- simple foreign keys to specific target collection
- easy per-collection cleanup
- easy generated migrations
- good fit for schema-driven CMS
- no need for polymorphic constraints

Cons:

- duplicated identity table shape for every collection
- cross-collection tooling needs dynamic table names
- global document search/workflow is harder

Alternative global table:

```plain text
documents
---------
document_id uuid primary key
collection_type text not null
status text not null
```


Pros:

- one place for workflow, permissions, locks, schedules
- relation target can reference one stable table
- easier global draft review queue

Cons:

- harder to enforce target collection with pure FK
- needs application validation or composite keys
- more abstract

For your architecture, I would choose **per-collection identity tables first**. It fits generated schema/migrations well.

---

## One subtle issue: relation FK points to the target identity table

If each relation table targets one known collection, then per-collection identity tables are excellent.

Example:

```plain text
article_categories.target_document_id
  -> category_documents.document_id
```


This is type-safe at the database level.

But for polymorphic/dynamic relations, per-collection identity tables become harder. Then you would need either:

```plain text
target_collection
target_document_id
```


or a global identity table.

If your relations are statically defined in schema, per-collection identity tables are better.

---

## What about status for non-draft collections?

For collection types without draft-and-publish, you have two options.

### Option A: Still create identity table with status

```plain text
status = PUBLISHED
```


Always.

This keeps the model uniform.

### Option B: Skip identity table

Relations to non-draft collections can still point directly to the content table.

I would not choose this because it creates two relation models.

I recommend keeping identity tables for all collection types:

```plain text
document identity exists for every collection
content rows exist for every collection
relations always point to document identity
```


For non-draft collections:

```plain text
status = PUBLISHED
```


and there is only one content row.

---

## Should identity status be exposed in API?

Yes.

Your response currently has publication metadata like `publishedAt` for published rows, but for editor UI you need a logical document status.

I would expose both concepts:

```json
{
  "id": 42,
  "documentId": "4ed4f3b0-...",
  "status": "MODIFIED",
  "publishedAt": "2026-05-01T10:00:00Z",
  "revision": 3,
  "title": "Draft title"
}
```


Where:

- `status` comes from the identity row
- `publishedAt`/`revision` comes from the selected content row if it is published, or maybe from the last published row if you need richer UI

For editor UI, you may eventually want:

```json
{
  "documentId": "4ed4f3b0-...",
  "status": "MODIFIED",
  "selectedVersion": "DRAFT",
  "publishedVersion": {
    "revision": 3,
    "publishedAt": "2026-05-01T10:00:00Z"
  }
}
```


But you can start simpler.

---

## Important invariant

If you store status, enforce this invariant:

```plain text
DRAFT     => draft row exists, published row does not exist
PUBLISHED => published row exists, draft row does not exist
MODIFIED  => both draft row and published row exist
```


That invariant should be maintained in the same database transaction as all lifecycle operations.

If status and rows can diverge, bugs will be painful.

---

## Final recommendation

Yes, use a document identity table per collection type.

But use at least:

```plain text
DRAFT
PUBLISHED
MODIFIED
```


not only:

```plain text
PUBLISHED
MODIFIED
```


Recommended model:

```plain text
{collection}_documents
- document_id primary key
- status: DRAFT | PUBLISHED | MODIFIED
- document-level metadata

{collection}
- id primary key
- document_id references {collection}_documents(document_id)
- publication_state: DRAFT | PUBLISHED
- content fields
- version/revision metadata

{owner}_{relation}
- owning_id references {owner}(id)
- target_document_id references {target}_documents(document_id)
```


This is probably the cleanest model for your use case:

- owner controls relation draft/publish state
- inverse side points to logical document identity
- relation FK integrity is restored
- editor UI can cheaply show `published` / `modified` / `draft`
- public API can still resolve only published target rows
- publishing becomes a controlled sync from draft owner row to published owner row