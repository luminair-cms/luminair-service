# Database Structure

This document describes how Luminair generates database schema from document types using the `migration` crate.

## Overview

The migration logic builds two kinds of tables from each document schema:

- **Document identity table** for document instances due to draft-and-publish
- **Collection table** for document data fields and lifecycle metadata
- **Relation tables** for owning-side relations between document types

The table generation is implemented in `migration/src/domain/mod.rs` using `MainTableBuilder` and `RelationTablesBuilder`.

## Document Identity Tables

Each document type produces one main table named after the normalized ID plus `_documents` suffix.
Example: a document with ID `partner-categories` uses table name `partner_categories_documents`.

Model:

{collection}_documents
- document_id primary key
- status: DRAFT | PUBLISHED | MODIFIED
- document-level metadata (created_at, created_by_id)

### Collection table

Name of this table is derived from the document type's normalized ID.
Example: a document with ID `partner-categories` uses table name `partner_categories`.

**Model:**

{collection}
- id primary key
- document_id references {collection}_documents(document_id)
- publication_state: doesn't exists, derived from published_at field
- content fields
- version metadata fields
- publication state fields (in case of draft-and-publish)

**Version metadata**

      updated_at timestamptz NOT NULL,
      updated_by_id text NULL,
      version integer NOT NULL,

**Publication state (in case of draft-and-publish):**
   
      revision integer NOT NULL,
      published_at timestamptz NULL,
      published_by_id text NULL,

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

### Main table indexes

A non-unique index is created on `document_id` for every main document table.

Constraints:

```sql
CREATE UNIQUE INDEX articles_one_draft_per_document
ON articles (document_id)
WHERE published_at IS NULL;

CREATE UNIQUE INDEX articles_one_published_per_document
ON articles (document_id)
WHERE published_at IS NOT NULL;
```

where `articles` is the main table name.

## Relation Tables

Relation tables are generated only for owning-side relations declared in the document schema. The owning relation types are:

- `hasOne`
- `hasMany`

For each owning relation, the migration crate creates a dedicated relation table named:

```text
{main_table_name}_{relation_attribute_name}_relation
```

### Relation table columns

Each relation table contains:

- `relation_id` — primary key, `serial`
- `owning_id` — `integer`, foreign key to the owning document's main table `id`
- `target_document_id references` — `uuid`, foreign key to {target}_documents(document_id)

### Relation lifecycle with draft-and-publish

When `draftAndPublish` is enabled, the same document instance is identified by `document_id`, but the database may contain multiple main table rows for the same instance over time:

- one row represents the published version
- another row represents the current draft version

Because relation tables join by concrete main table row IDs (`owning_id` and `inverse_id`), connecting or disconnecting documents happens against a specific row version.

- **Connect**: add a relation row for the draft/main row that is currently being edited.
- **Disconnect**: remove the relation row from the draft/main row representing the next state.

This means relation changes are versioned implicitly by row identity, not by `document_id`.
A published document can keep its last-live relation rows until the next publish action applies draft changes.

In practice:

- The published row and draft row share the same `document_id`.
- The relation table stores `owning_id` and `inverse_id` values that reference a specific main row.
- Draft-time relation updates should be made against the draft row.
- Publishing should synchronize the published row with the draft row, including relation additions and removals.

If `draftAndPublish` is disabled, there is only one main row per document instance and relations are managed directly on that single row.

## Migration Strategy

The migration system compares the target schema (derived from document configuration) with the actual database schema and generates DDL statements to reconcile them. The migration process is idempotent and only executes changes when needed.

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
