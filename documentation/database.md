# Database Structure

This document describes how Luminair generates database schema from document types using the `migration` crate.

## Overview

The migration logic builds the following tables from each document schema:

- **Main table** for working/draft document content and lifecycle metadata
- **Snapshots table** for immutable published snapshots of document content
- **Relation table** for working/draft relations between document types
- **Snapshot relation table** for relations of frozen published snapshots

The table generation is implemented in `migration/src/domain/mod.rs` using `MainTableBuilder` and `RelationTablesBuilder`.

## Core Tables Pattern

Luminair uses a **main table + snapshots table** database schema for each document type.

### Common columns

These columns exist both in main table and in snapshot table.

- `revision` — `integer` NOT NULL DEFAULT 0 (last published revision index, 0 if never published)
- `created_at` — `timestamptz` NOT NULL DEFAULT now()
- `updated_at` — `timestamptz` NOT NULL DEFAULT now()
- `created_by_id` — `text` NULL
- `updated_by_id` — `text` NULL 
- `published_at` — `timestamptz` NULL (timestamp of last publish)
- `published_by_id` — `text` NULL (user ID of publisher)

### Main Table: `{collection}`

Name of this table is derived from the document type's normalized ID.
Example: a document with ID `partner-categories` uses table name `partner_categories`.

The main table contains the current working draft (or the last published version if no edits have been made) along with metadata and content fields.

**Columns:**
- `document_id` — `uuid` PRIMARY KEY
- Common columns
- `status` — `text` NOT NULL CHECK (status IN ('DRAFT', 'PUBLISHED', 'MODIFIED'))
- `version` — `integer` NOT NULL DEFAULT 1 (increments on every save/edit)
- Content columns (dynamic, based on schema fields)

### Snapshots Table: `{collection}_snapshots`

Name of this table is derived from the normalized ID plus `_snapshots` suffix (e.g., `partner_categories_snapshots`).

It stores immutable published snapshots. Every publish action inserts a new row copying the main table's content fields. (for history functionality, in MVP will be only one row in snapshots table, with last published document version; later we can add functionality to keep all published versions)

**Columns:**
- `snapshot_id` — `bigserial` PRIMARY KEY
- `document_id` — `uuid` NOT NULL REFERENCES `{collection}`(document_id) ON DELETE CASCADE
- Common columns
- Content columns (dynamic copy of schema fields at publish time)

**Indexes & Constraints:**
- `UNIQUE (document_id, revision)` constraint ensures audit/history integrity.

### Field columns

Document fields are converted to columns according to the field type mapping in `infer_column_type()`:

- `uid` → `text`
- `uuid` → `uuid`
- `text` → `text`
- `localizedText` → `jsonb`
- `integer` → `integer` (size preserved as `Int16`, `Int32`, or `Int64`)
- `decimal` → `decimal(precision, scale)`
- `date` → `date`
- `dateTime` → `timestamp with time zone`
- `boolean` → `boolean`
- `json` → `jsonb`

Field column names are derived from schema attribute IDs.
Each field column preserves the schema's `required` and `unique` flags.

---

Example for an `articles` type:

#### Main Table: `articles`
```sql
CREATE TABLE articles (
    document_id     uuid PRIMARY KEY,
    status          text NOT NULL CHECK (status IN ('DRAFT', 'PUBLISHED', 'MODIFIED')),
    version         integer NOT NULL,
    -- publication revision, only changes when document is published  
    revision        integer NOT NULL DEFAULT 0, 
    -- audit trail columns
    created_at      timestamptz NOT NULL,
    created_by_id   text NULL,
    updated_at      timestamptz NOT NULL,
    updated_by_id   text NULL,

    -- publication metadata (redundant to enable single-table queries)
    published_at    timestamptz NULL,
    published_by_id text NULL,

    -- current working content fields
    title           text NULL,
    body            text NULL
);
```

#### Snapshots Table: `article_snapshots`
```sql
CREATE TABLE article_snapshots (
    snapshot_id     bigserial PRIMARY KEY,
    document_id     uuid NOT NULL REFERENCES articles(document_id) ON DELETE CASCADE,
    -- publication revision, only changes when document is published  
    revision        integer NOT NULL DEFAULT 0, 
    -- audit trail columns, copied from main table
    created_at      timestamptz NOT NULL,
    created_by_id   text NULL,
    updated_at      timestamptz NOT NULL,
    updated_by_id   text NULL,

    -- publication metadata (redundant to enable single-table queries)
    published_at    timestamptz NULL,
    published_by_id text NULL,

    -- immutable snapshot of content at publish time
    title           text NULL,
    body            text NULL,

    UNIQUE (document_id, revision)
);
  
```

## Relation Tables

Relation tables are generated only for owning-side relations declared in the document schema. The owning relation types are:

- `hasOne`
- `hasMany`

For each owning relation, the migration crate creates a dedicated pair of relation tables to track working and published relations separately.

### Working Relations: `{collection}_{relation_name}_relation`

Used by editor APIs to read and write draft/working relations.

**Columns:**
- `owning_document_id` — `uuid` NOT NULL REFERENCES `{collection}`(document_id) ON DELETE CASCADE
- `target_document_id` — `uuid` NOT NULL REFERENCES `{target_collection}`(document_id) ON DELETE CASCADE
- PRIMARY KEY (`owning_document_id`, `target_document_id`)

### Snapshot Relations: `{collection}_{relation_name}_relation_snapshots`

Used by public APIs to query relations of frozen published snapshots.

**Columns:**
- `snapshot_id` — `bigint` NOT NULL REFERENCES `{collection}_snapshots`(snapshot_id) ON DELETE CASCADE
- `target_document_id` — `uuid` NOT NULL REFERENCES `{target_collection}`(document_id) ON DELETE CASCADE
- `owning_document_id` — `uuid` NOT NULL REFERENCES `{collection}`(document_id) ON DELETE CASCADE
- PRIMARY KEY (`snapshot_id`, `target_document_id`)

### Relation lifecycle with draft-and-publish

Because relation tables link by UUIDs (`owning_document_id` and `target_document_id`), editing relations happens directly against the working copy:

- **Connect**: add a row to `{collection}_{relation_name}_relation`.
- **Disconnect**: remove the row from `{collection}_{relation_name}_relation`.
- **Publish**: inside a single transaction, the publish operation inserts a new row in `{collection}_snapshots` (returning `snapshot_id`), then copies all matching relation rows from the working relation table to the snapshot relation table under that `snapshot_id`:
  ```sql
  INSERT INTO article_categories_relation_snapshots (snapshot_id, target_document_id, owning_document_id)
  SELECT $snapshot_id, target_document_id, owning_document_id
  FROM article_categories_relation
  WHERE owning_document_id = $document_id;
  ```

If `draftAndPublish` is disabled, the snapshots and snapshot relations tables are still created for uniformity, but documents are immediately published (a snapshot is created immediately on save) and only the main/snapshot table pairs are queried.

### Polymorphic Relations (Post-MVP)

While polymorphic relations are **excluded from the MVP**, the architecture for post-MVP implementation is specified as follows:

1. **Junction Table Design:**
   Rather than using a generic, type-unsafe string column (such as `target_type` + `target_id`), the relation table will contain **explicit, nullable foreign key columns** targeting each possible target collection's main table.

   For example, if `article_related_content_relation` can link to `articles` or `categories`:
   ```sql
   CREATE TABLE article_related_content_relation (
       relation_id serial PRIMARY KEY,
       owning_document_id uuid NOT NULL REFERENCES articles(document_id) ON DELETE CASCADE,
       
       -- Polymorphic target columns referencing target main tables
       article_target_document_id uuid NULL REFERENCES articles(document_id) ON DELETE CASCADE,
       category_target_document_id uuid NULL REFERENCES categories(document_id) ON DELETE CASCADE,
       
       -- Database-level check constraint to ensure mutual exclusivity
       CONSTRAINT check_only_one_target CHECK (
           (article_target_document_id IS NOT NULL)::int + 
           (category_target_document_id IS NOT NULL)::int = 1
       )
   );
   ```

2. **Benefits of this Pattern:**
   - **Strict Referential Integrity:** Retains native PostgreSQL foreign keys for all polymorphic targets.
   - **Database-Level Cascades:** Deleting a target document automatically and cleanly cascade-deletes all polymorphic relation records pointing to it.
   - **Type Safety:** Strong relational integrity at the SQL level.

The migration system compares the target schema (derived from document configuration) with the actual database schema and generates DDL statements to reconcile them. The migration process is idempotent and only executes changes when needed.

> [!NOTE]
> Column-level migrations (e.g., adding, removing, or modifying individual fields/columns on existing tables) are **not implemented in the MVP**. Schema updates at the field/column level require recreating the collection tables or manual DDL intervention. Only table-level additions and removals are processed.

### MVP: Core Migration Cases

#### Case 1: Adding a New Collection

Implementation location: `migration/src/domain/migration.rs` - `CreateTableStep`

#### Case 2: Removing an Existing Collection

When a document type is removed from the schema configuration:

1. **Orphaned relation cleanup**: All relation tables referencing the removed collection are dropped first (cascade handling via foreign keys).
2. **Main table removal**: The main collection table is dropped.
3. **Data preservation option**: Pre-migration backups are recommended before removing collections (responsibility of deployment pipeline).

#### Case 3: Adding a New Relation Between Collections

When a new relation is declared in a document's schema (e.g., `hasOne` or `hasMany`):

Implementation location: `migration/src/domain/mod.rs` - `RelationTablesBuilder`

#### Case 4: Removing an Existing Relation Between Collections

When a relation is removed from a document's schema:

1. **Foreign key removal**: Constraints referencing both collections are dropped.
2. **Relation table removal**: The relation table is dropped, cascading any dependent data.
3. **Dependent data cleanup**: All relation records are automatically removed via CASCADE.

### Migration Execution

The `migration` crate is run as a standalone binary during deployment:

```bash
cargo run --manifest-path migration/Cargo.toml
```

The migration process:
1. Loads document schemas from `config/schema/` directory
2. Establishes database connection
3. Loads existing database schema
4. Compares the necessary schema vs. actual schema
5. Generates and executes DDL statements for differences
6. Logs migration progress

### Migration Safety

- **Idempotency**: Running migrations multiple times is safe; only missing objects are created.
- **Cascade handling**: Foreign keys use ON DELETE CASCADE to prevent orphaned data.
- **Transaction safety**: All DDL statements are executed within transactions (handled by database driver).
- **No data loss on adds**: Adding collections and relations preserves existing data.
- **Data loss on removes** (Case 2 & 4): Removing collections or relations will delete associated data; this requires explicit schema changes.

### Key implementation files

- `migration/src/domain/mod.rs`
- `migration/src/domain/tables.rs`
- `migration/src/domain/migration.rs`
- `luminair_common/src/domain/entities.rs`

These files define how document schemas are translated into database tables, columns, foreign keys, and indexes.
