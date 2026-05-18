# Database Structure

This document describes how Luminair generates database schema from document types using the `migration` crate.

## Overview

The migration logic builds two kinds of tables from each document schema:

- **Main document table** for document fields and lifecycle metadata
- **Relation tables** for owning-side relations between document types

The table generation is implemented in `migration/src/domain/mod.rs` using `MainTableBuilder` and `RelationTablesBuilder`.

## Main Document Table

Each document type produces one main table named after the normalized document ID.

Example: a document with ID `partner-categories` uses table name `partner_categories`.

### Main table columns

All main tables include these base columns:

- `id` — primary key, `serial`
- `document_id` — `uuid`, identifies the document instance
- `created_at` — `timestamp with time zone`, defaults to `now()`
- `updated_at` — `timestamp with time zone`
- `created_by_id` — `text`
- `updated_by_id` — `text`
- `version` — `integer`

In addition, when `draftAndPublish` is enabled for a document type, the following publication columns are added:

- `published_at` — `timestamp with time zone`
- `published_by_id` — `text`
- `revision` — `integer`

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

Each field column preserves the schema's `required` and `unique` flags.

### Main table indexes

A non-unique index is created on `document_id` for every main document table.

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
- `inverse_id` — `integer`, foreign key to the related document's main table `id`

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

### Foreign keys and indexes

Each relation table includes:

- foreign key from `owning_id` to the owning table's `id`
- foreign key from `inverse_id` to the inverse/target table's `id`
- index on `owning_id`
- index on `inverse_id`

## Naming conventions

- Main table names are derived from the document type's normalized ID.
- Relation table names append the relation attribute name and `_relation` suffix.
- Field column names are derived from schema attribute IDs.

## Example

Given a `partners` document with an owning relation attribute `brands`, migration creates:

- Main table: `partners`
- Relation table: `partners_brands_relation`

The relation table links `partners.id` to `brands.id` through `owning_id` and `inverse_id`.

## Migration Strategy

The migration system compares the target schema (derived from document configuration) with the actual database schema and generates DDL statements to reconcile them. The migration process is idempotent and only executes changes when needed.

### MVP: Core Migration Cases

#### Case 1: Adding a New Collection

When a new document type is added to the schema configuration:

1. **Main table creation**: A new main table is created with all base columns (id, document_id, created_at, updated_at, etc.) and field-specific columns.
2. **Index creation**: A non-unique index is created on `document_id` for efficient queries.
3. **Publication columns** (if enabled): If the document has `draftAndPublish` enabled, publication columns (`published_at`, `published_by_id`, `revision`) are included.

Example DDL flow:
```sql
CREATE TABLE "public"."new_collection" (
  "id" SERIAL,
  "document_id" UUID NOT NULL,
  "field1" TEXT,
  ...
  PRIMARY KEY(id)
);
CREATE INDEX "new_collection_document_id_idx" ON "public"."new_collection" ("document_id");
```

Implementation location: `migration/src/domain/migration.rs` - `CreateTableStep`

#### Case 2: Removing an Existing Collection

When a document type is removed from the schema configuration:

1. **Orphaned relation cleanup**: All relation tables referencing the removed collection are dropped first (cascade handling via foreign keys).
2. **Main table removal**: The main collection table is dropped.
3. **Data preservation option**: Pre-migration backups are recommended before removing collections (responsibility of deployment pipeline).

Expected DDL flow:
```sql
DROP TABLE IF EXISTS "public"."collection_relation_name_relation" CASCADE;
DROP TABLE "public"."collection_name" CASCADE;
```

#### Case 3: Adding a New Relation Between Collections

When a new relation is declared in a document's schema (e.g., `hasOne` or `hasMany`):

1. **Relation table creation**: A dedicated relation table named `{main_table}_{relation_name}_relation` is created.
2. **Foreign key constraints**: Two foreign keys are created:
   - `owning_id` → owning collection's main table `id` (ON DELETE CASCADE)
   - `inverse_id` → related collection's main table `id` (ON DELETE CASCADE)
3. **Index creation**: Indexes are created on both `owning_id` and `inverse_id` for query performance.

Example DDL flow:
```sql
CREATE TABLE "public"."partners_brands_relation" (
  "relation_id" SERIAL,
  "owning_id" INT NOT NULL,
  "inverse_id" INT NOT NULL,
  PRIMARY KEY(relation_id)
);
ALTER TABLE "public"."partners_brands_relation" ADD CONSTRAINT "partners_brands_relation_owning_id_fkey"
  FOREIGN KEY ("owning_id") REFERENCES "public"."partners" ("id") ON DELETE CASCADE;
ALTER TABLE "public"."partners_brands_relation" ADD CONSTRAINT "partners_brands_relation_inverse_id_fkey"
  FOREIGN KEY ("inverse_id") REFERENCES "public"."brands" ("id") ON DELETE CASCADE;
CREATE INDEX "partners_brands_relation_owning_id_idx" ON "public"."partners_brands_relation" ("owning_id");
CREATE INDEX "partners_brands_relation_inverse_id_idx" ON "public"."partners_brands_relation" ("inverse_id");
```

Implementation location: `migration/src/domain/mod.rs` - `RelationTablesBuilder`

#### Case 4: Removing an Existing Relation Between Collections

When a relation is removed from a document's schema:

1. **Foreign key removal**: Constraints referencing both collections are dropped.
2. **Relation table removal**: The relation table is dropped, cascading any dependent data.
3. **Dependent data cleanup**: All relation records are automatically removed via CASCADE.

Expected DDL flow:
```sql
ALTER TABLE "public"."partners_brands_relation" DROP CONSTRAINT "partners_brands_relation_inverse_id_fkey";
ALTER TABLE "public"."partners_brands_relation" DROP CONSTRAINT "partners_brands_relation_owning_id_fkey";
DROP TABLE "public"."partners_brands_relation" CASCADE;
```

### Migration Execution

The `migration` crate is run as a standalone binary during deployment:

```bash
cargo run --manifest-path migration/Cargo.toml
```

The migration process:
1. Loads document schemas from `config/schema/` directory
2. Establishes database connection
3. Loads existing database schema
4. Compares needed schema vs. actual schema
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
