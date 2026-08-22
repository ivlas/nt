# nt Architecture

`nt` is a deliberately single-purpose note application. It is one Cargo package
with direct, compile-time dependencies rather than traits, plugins, registries,
or dependency-injection machinery.

## Dependency Direction

The primary command path is:

```text
CLI -> commands -> note model/repository -> SQLite storage
```

`src/lib.rs` is the process composition root and `run_process` is the only
intentional public Rust entry point. It resolves process state, builds the
application context, and dispatches the parsed command. Note, repository, CLI,
and schema APIs remain implementation details rather than a supported SDK.

The source layout is:

```text
src/
  note/
    body.rs
    collection.rs
    date.rs
    id.rs
    model.rs
    query.rs
    schema.rs
    repository/
  storage/
  cli/
  commands/
  app.rs
  schema.rs
  error.rs
  lib.rs
  main.rs
```

`src/error.rs` defines the concrete crate-wide error vocabulary, including the
stable process categories consumed by the binary adapter.

## Responsibilities

`src/cli/` owns command grammar, body input, editor execution, rendering,
terminal behavior, help, and canonical home-directory resolution.

`src/commands/` owns orchestration. Handlers parse note values, select the
appropriate concrete repository operation, and render the result without
issuing SQL.

`src/note/` is the business boundary. It owns note identity and validation,
collections, tags, CommonMark body rules, the query model, concrete persistence
operations, rehydration, optimistic body-edit conflicts, and note schema SQL.
`Repository` is a concrete facade, not a generic abstraction.

`src/storage/` owns SQLite infrastructure: opening and configuring connections,
foreign keys, WAL, busy handling, private filesystem setup, atomic publication
of newly initialized databases, and exact schema-validation mechanics. It does
not own note models or note schema SQL.

`src/app.rs` carries the concrete, testable process dependencies used by command
handlers. `src/schema.rs` binds the fixed nt database identity and note schema to
the storage mechanics and supplies storage-backed `Repository` construction.

The production dependency rules are:

```text
storage does not import note
note does not import CLI, commands, or App
commands orchestrate concrete note and storage operations
CLI parses and renders note-facing values
```

Tests may construct the complete application across these boundaries to verify
integration behavior.

## Schema Ownership

`src/schema.rs` owns the application ID, schema version, and one concrete schema
manifest. The manifest contains a flat ordered object list, required FTS
shadow-table names, and allowed trigger names. There is no schema fragment
composition or runtime registration model.

`src/note/schema.rs` owns the exact SQL for the version table, all note tables,
the FTS virtual table, triggers, and indexes, together with the FTS shadow-table
and trigger requirements. `src/storage/schema_engine.rs` consumes the manifest
to perform transactional initialization, identity inspection, exact object
validation, version checks, and unknown-trigger rejection.

Version-1 SQL definitions are a compatibility boundary. Structural refactors
must leave stored `sqlite_schema.sql` values and creation order unchanged. The
independent fixture in `tests/fixtures/v1_schema.sql` protects that contract
through the compiled binary interface.

## Product Boundary

`nt` has notes only. Search improvements, import and export, and similar work are
capabilities around notes, not new first-class domains.

Sources and generic metadata are not part of the note model. External resources,
bookmarks, imported documents, and agent-generated summaries can instead be
ordinary CommonMark notes organized by collections, tags, and directional note
links. They receive no reserved note kinds or hidden semantics.

Append-only memory and other distinct product models are outside `nt` and belong
in separate applications. The codebase reserves no extension point, directory,
behavior, or storage for them.
