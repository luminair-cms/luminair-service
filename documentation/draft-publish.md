# Draft & Publish Database Design

This document details the database schemas, SQL query patterns, and relation versioning mechanisms used to implement the Draft & Publish workflow in the `luminair-service` persistence layer.

---

Luminair separates draft states from published snapshots using a **Main Table + Snapshots Table** pattern.

> [!NOTE]
> The snapshots table `{collection}_snapshots` and its corresponding relation snapshot tables are **only created and used if `draftAndPublish` is enabled** on the collection. If `draftAndPublish` is disabled (OFF), snapshot tables are not generated, and all read and write queries are executed directly on the main table `{collection}`.

### 1. Main Table: `{collection}`
Contains the current working draft (or the last published version if no edits have been made) along with metadata and content fields.
* **Columns**:
  * `document_id` — `uuid` PRIMARY KEY
  * `status` — `text` (`DRAFT`, `PUBLISHED`, `MODIFIED`)
  * `version` — `integer` NOT NULL DEFAULT 1 (increments on every edit or save)
  * `revision` — `integer` (keeps the latest published revision number)
  * `created_at`, `updated_at`, `published_at` — timestamps

### 2. Snapshots Table: `{collection}_snapshots`
Stores immutable published snapshots of the document. Every publish action inserts a new row copying the main table's content fields.
* **Columns**:
  * `snapshot_id` — `bigserial` PRIMARY KEY
  * `document_id` — `uuid` NOT NULL REFERENCES `{collection}`(document_id) ON DELETE CASCADE
  * `revision` — `integer` NOT NULL
  * `published_at` — timestamp
  * `created_at`, `updated_at` — timestamps
  * Content columns (dynamic copy of schema fields at publish time)

---

## SQL Database Operations

### 1. Create Document
```sql
INSERT INTO articles (document_id, status, created_at, updated_at, version, revision, published_at)
VALUES ($document_id, 'DRAFT', now(), now(), 1, 0, NULL);
```

### 2. Edit Document
```sql
UPDATE articles 
SET updated_at = now(), 
    version = version + 1,
    status = CASE WHEN revision = 0 THEN 'DRAFT' ELSE 'MODIFIED' END,
    published_at = NULL,
    title = $title,
    body = $body
WHERE document_id = $document_id;
```

### 3. Publish Document
* **Step 1: Insert published snapshot**
  ```sql
  INSERT INTO article_snapshots (document_id, revision, published_at, title, body)
  VALUES ($document_id, $next_revision, now(), $title, $body)
  RETURNING snapshot_id;
  ```
* **Step 2: Update main table status and version metadata**
  ```sql
  UPDATE articles 
  SET status = 'PUBLISHED', 
      version = version + 1,
      revision = $next_revision,
      published_at = now(),
      updated_at = now()
  WHERE document_id = $document_id;
  ```

---

## Relations Versioning Pattern

To keep draft relations separate from published relations, the database utilizes relation table pairs when `draftAndPublish` is enabled. If `draftAndPublish` is disabled, only the Working Relations table is created and queried.

### 1. Working Relations (Draft)
```sql
CREATE TABLE article_categories_relation (
    owning_document_id  uuid NOT NULL REFERENCES articles(document_id) ON DELETE CASCADE,
    target_document_id  uuid NOT NULL REFERENCES categories(document_id) ON DELETE CASCADE,
    PRIMARY KEY (owning_document_id, target_document_id)
);
```

### 2. Published Relations (Snapshots)
```sql
CREATE TABLE article_categories_relation_snapshots (
    snapshot_id        bigint NOT NULL REFERENCES article_snapshots(snapshot_id) ON DELETE CASCADE,
    target_document_id uuid NOT NULL REFERENCES categories(document_id) ON DELETE CASCADE,
    owning_document_id uuid NOT NULL REFERENCES articles(document_id) ON DELETE CASCADE,
    PRIMARY KEY (snapshot_id, target_document_id)
);
```

### 3. Publishing Relations
When publishing the owning document, we freeze the draft relations into the relation snapshot table:
```sql
INSERT INTO article_categories_relation_snapshots (snapshot_id, target_document_id, owning_document_id)
SELECT $snapshot_id, target_document_id, owning_document_id
FROM article_categories_relation
WHERE owning_document_id = $document_id;
```

---

## Query Patterns

### Reading Published Content (Public API)
```sql
SELECT 
    s.snapshot_id,
    s.document_id,
    s.revision,
    s.published_at,
    s.title,
    s.body,
    sc.target_document_id
FROM article_snapshots s
LEFT JOIN article_categories_relation_snapshots sc ON sc.snapshot_id = s.snapshot_id
WHERE s.document_id = $1;
```

### Reading Working Content (Editor API)
```sql
SELECT 
    a.document_id,
    a.status,
    a.created_at,
    a.updated_at,
    a.version,
    a.revision,
    a.published_at,
    a.title,
    a.body,
    ac.target_document_id
FROM articles a
LEFT JOIN article_categories_relation ac ON ac.owning_document_id = a.document_id
WHERE a.document_id = $1;
```
