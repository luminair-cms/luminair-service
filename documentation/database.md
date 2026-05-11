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

## Key implementation files

- `migration/src/domain/mod.rs`
- `migration/src/domain/tables.rs`
- `luminair_common/src/domain/entities.rs`

These files define how document schemas are translated into database tables, columns, foreign keys, and indexes.
