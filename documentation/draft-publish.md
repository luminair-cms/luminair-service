# Draft and Publish Workflow

## Overview

Luminair implements a draft-and-publish workflow for document types that have `draftAndPublish` enabled in their schema options. This design uses a **main table + snapshots table** pattern to cleanly separate working/draft state from immutable published snapshots.

## Core Architecture

### Main Table Pattern

Each document type with draft-and-publish support has:
1. **Main table** — contains identity, current working copy, and audit metadata
2. **Snapshots table** — immutable published snapshots, indexed by document and revision

See [database](database.md) for detailed database schema and examples.

### Key Concepts

- **Main Table** (`articles`): Single row per document, containing the identity and working/draft content. Mutated by editors.
- **Snapshots Table** (`article_snapshots`): Immutable, append-only. Contains all historical published versions.
- **status**: Tracks the document's current state:
  - `DRAFT` — Never published, working on initial draft
  - `PUBLISHED` — Has published snapshot(s), no pending changes
  - `MODIFIED` — Has published snapshot(s), with pending draft changes
- **version**: Incremented on every save (edit, publish). Tracks total modification count.
- **revision**: Present only in snapshots. Publication sequence number (1, 2, 3, …).

## Document Lifecycle

### State Transitions

```
Create:  status=DRAFT,     version=1, revision=0, published_at=NULL
Edit:    status=DRAFT,     version=2, revision=0, published_at=NULL
Publish: status=PUBLISHED, version=3, revision=1, published_at=now()   ← snapshot revision=1 created
Edit:    status=MODIFIED,  version=4, revision=1, published_at=NULL    ← edits to main, snapshot unchanged
Edit:    status=MODIFIED,  version=5, revision=1, published_at=NULL
Publish: status=PUBLISHED, version=6, revision=2, published_at=now()   ← snapshot revision=2 created
```

### Operations

***Note: Some fields can be omitted in SQL examples, for full structure see `database.md`***
***The `$placeholder` values should be replaced with actual values in production***

#### Create Document
```sql
INSERT INTO articles (document_id, status, created_at, updated_at, version, revision, published_at, published_by_id)
VALUES ($document_id, 'DRAFT', now(), now(), 1, 0, NULL, NULL);
```

#### Edit Document
```sql
UPDATE articles 
SET updated_at = now(), 
    updated_by_id = $user_id, 
    version = version + 1,
    status = CASE WHEN revision = 0 THEN 'DRAFT' ELSE 'MODIFIED' END,
    published_at = NULL,
    published_by_id = NULL,
    title = $title,
    body = $body
WHERE document_id = $document_id;
```

#### Publish Document

**Step 1:** Insert snapshot
```sql
INSERT INTO article_snapshots (document_id, revision, published_at, published_by_id, title, body)
VALUES ($document_id, $next_revision, now(), $user_id, $title, $body)
RETURNING snapshot_id;
```

**Step 2:** Update main table status, version, and publication metadata
```sql
UPDATE articles 
SET status = 'PUBLISHED', 
    version = version + 1,
    revision = $next_revision,
    published_at = now(),
    published_by_id = $user_id,
    updated_at = now(),
    updated_by_id = $user_id
WHERE document_id = $document_id;
```

## Relations Pattern

Relations follow the same main + snapshots pattern, keeping draft and published relation sets separate and immutable.

### Working Relations (Draft)
```sql
CREATE TABLE article_categories (
    owning_document_id  uuid NOT NULL REFERENCES articles(document_id) ON DELETE CASCADE,
    target_document_id  uuid NOT NULL REFERENCES categories(document_id) ON DELETE CASCADE,
    PRIMARY KEY (owning_document_id, target_document_id)
);
```

### Published Relations (Snapshots)
```sql
CREATE TABLE article_snapshot_categories (
    snapshot_id        bigint NOT NULL REFERENCES article_snapshots(snapshot_id) ON DELETE CASCADE,
    target_document_id uuid NOT NULL REFERENCES categories(document_id) ON DELETE CASCADE,
    PRIMARY KEY (snapshot_id, target_document_id)
);
```

### Relation Tables Pairing

- `articles` ↔ `article_categories` (working/draft relation set)
- `article_snapshots` ↔ `article_snapshot_categories` (published relation set)

### Relation Operations

#### Add to Draft Relations
```sql
INSERT INTO article_categories (owning_document_id, target_document_id)
VALUES ($document_id, $target_document_id);
```

#### Publish Relations

When publishing, freeze the current draft relations into the snapshot:

```sql
INSERT INTO article_snapshot_categories (snapshot_id, target_document_id)
SELECT $snapshot_id, target_document_id
FROM article_categories
WHERE owning_document_id = $document_id;
```

## Query Patterns

### Key Principle

**No mixed states.** Each query touches exactly one pair of tables (main + relations OR snapshots + snapshot-relations) and never needs a `status` filter.

### Reading Published Content (Public API)

```sql
SELECT 
    s.snapshot_id,
    s.document_id,
    s.revision,
    s.published_at,
    s.published_by_id,
    s.title,
    s.body,
    sc.target_document_id
FROM article_snapshots s
LEFT JOIN article_snapshot_categories sc ON sc.snapshot_id = s.snapshot_id
WHERE s.document_id = $1
  AND s.revision = (
      SELECT MAX(revision) FROM article_snapshots WHERE document_id = $1
  );
```

This query:
- Reads only snapshots (immutable, published content)
- No `status` filter needed — snapshots table contains only published content
- Efficient for public API endpoints
- Can be cached indefinitely per snapshot

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
    a.published_by_id,
    a.title,
    a.body,
    ac.target_document_id
FROM articles a
LEFT JOIN article_categories ac ON ac.owning_document_id = a.document_id
WHERE a.document_id = $1;
```

This query:
- Reads only main tables (working content)
- All draft, MODIFIED, and DRAFT content visible
- Used by editors to view and edit current working copy
- Can safely display alongside publication history from snapshots table

## API Considerations

### Content Endpoints

- **Public API** (`GET /articles?status=published` or `GET /articles/:id`): Reads from snapshots table, returns latest published version
- **Editor API** (`GET /articles?status=draft`): Reads from main table, returns the latest editorial state (draft row if unpublished changes exist, otherwise the published row)
- **History API** (`GET /articles/:id/revisions`): Reads from snapshots table, shows all published versions

### Status Field Usage

The `status` field on the main table indicates editorial workflow state:
- `DRAFT` — No published versions exist, document is still being created
- `PUBLISHED` — Document has one or more published versions, no pending changes
- `MODIFIED` — Document has published versions AND pending draft changes awaiting publish

### Filtering Documents

When listing documents, filter by purpose:

```sql
-- Show draft documents awaiting first publication
SELECT a.* FROM articles a WHERE a.status = 'DRAFT';

-- Show documents with pending changes
SELECT a.* FROM articles a WHERE a.status = 'MODIFIED';

-- Show all published documents (snapshot-based)
SELECT DISTINCT s.document_id, MAX(s.revision) as latest_revision
FROM article_snapshots s
GROUP BY s.document_id;
```

## Relations and Draft-Publish Behavior

### Principle: Relations are Versioned Per Document

- A relation change is part of the owning document's draft
- Publishing the owning document "approves" the relation change
- The related document's publication state is independent

### Editing Relations

Relations are always edited against the working copy:
```sql
-- Editor adds a category to draft
INSERT INTO article_categories (owning_document_id, target_document_id)
VALUES ($article_id, $category_id);
```

- The category can be in any state (DRAFT, PUBLISHED, MODIFIED)
- The relation change is not visible to the public until the article is published

### Publishing with Relations

When publishing, the relation set is frozen:
```sql
-- 1. Create snapshot (see Publish Document section)
-- 2. Freeze draft relations into snapshot
INSERT INTO article_snapshot_categories (snapshot_id, target_document_id)
SELECT $snapshot_id, target_document_id
FROM article_categories
WHERE owning_document_id = $document_id;
-- 3. Update main table status
```

After publish:
- Main table relations remain unchanged (for future editing)
- Snapshot captures the frozen relation set at publish time
- Public API reads snapshot relations for a stable, versioned view

## Schema Configuration

Document types enable draft-and-publish in their schema:

```json
{
  "options": {
    "draftAndPublish": true
  }
}
```

When `draftAndPublish` is `false` or omitted, a snapshots table is still created to keep the schema consistent and support published history, but documents are always considered published.

## Benefits of This Approach

- **Immutability**: Published snapshots are append-only and never mutated
- **Performance**: Public queries read only snapshots, enabling caching and optimization
- **Auditability**: Complete history available in snapshots table
- **Simplicity**: No complex enum states or publication state tracking logic
- **No mixed states**: Each query context (editor vs public) uses appropriate table pair
- **Efficient relations**: Frozen relation snapshots at publish time prevent query-time filtering