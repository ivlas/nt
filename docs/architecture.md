# nt Architecture

This document explains code ownership and execution boundaries. Product rules
are in [Design](design.md), and public behavior is in
[CLI Reference](cli-reference.md).

`nt` is one Cargo package with direct compile-time dependencies. It does not use
repository traits, plugins, registries, or dependency-injection frameworks.

## Dependency Direction

```text
CLI -> commands -> application schema -> SQLite storage
                -> note repository   -> opened SQLite connection
```

`src/lib.rs` is the process entry point. `run_process` resolves process state,
builds the concrete application context, and dispatches a parsed command. Other
Rust APIs are implementation details, not a supported SDK.

Production dependencies follow these rules:

```text
storage does not import note
note does not import CLI, commands, or App
application schema imports model schema definitions, not repositories
repositories receive connections opened by the application schema layer
commands construct the repository needed for one operation
```

## Responsibilities

| Area | Ownership |
| --- | --- |
| `src/cli/` | Command grammar, body input, note editor execution, rendering, terminal behavior, help, and home resolution |
| `src/commands/` | Command orchestration; parses values, opens storage, invokes one repository operation, and renders results |
| `src/note/` | Note values, validation, queries, persistence, edit conflicts, and note schema SQL |
| `src/storage/` | SQLite connections, foreign keys, WAL, busy handling, filesystem setup, atomic initialization, and schema validation mechanics |
| `src/schema.rs` | Application identity, complete schema manifest, initialization, and validated read-only or read-write opening |
| `src/app.rs` | Concrete process dependencies used to test command handlers without mutating global process state |
| `src/error.rs` | Crate-wide errors and stable process exit categories |

Command handlers do not issue SQL. The note `Repository` is a concrete
interface over an already-open connection. It does not resolve, initialize,
open, or validate the application database.

## Command Lifecycle

Read commands open a validated read-only connection, execute repository
operations, and render the result. Exact `show` commands write and flush the
canonical body. `read` uses one ordered repository statement to hydrate complete
notes, including sorted relationship aggregates, without per-note queries.
Arbitrary-ID reads pass the deduplicated set as one JSON value and join it
inside that statement, avoiding both per-ID SQL and host-parameter limits.
The `id:-` CLI form reads newline-delimited IDs from stdin before opening
storage, allowing large batches to bypass platform command-line limits.
`changes` traverses the compact change table by its revision-and-ID primary key.
Redirected note results stream rows; an intentionally closed downstream pipe
ends those commands successfully.

Multi-query reads may use a read transaction when they need one consistent
snapshot. Input-taking mutations collect and validate their body outside a
database connection where possible. Note editing reads the current body and
version, closes storage while the editor runs, then reopens storage and commits
only if the body version still matches. No transaction or connection remains
open while waiting for editor input. An explicit `if-rev:` precondition adds a
unified note-revision check, so metadata as well as body changes can invalidate
the edit while preserving the body-only behavior of unguarded editor sessions.

Each mutation performs one short repository transaction. The commit completes
before its success line is written, so a later output failure cannot roll back
the mutation. This boundary is surfaced explicitly by the CLI error contract.
Writable transactions acquire SQLite's writer lock with `BEGIN IMMEDIATE`
before incrementing the singleton global revision. The increment, canonical
changes, affected live-note revision stamps, change-feed rows, and derived FTS
changes commit or roll back together.
Optional mutation revision preconditions are read and compared after that
writer lock is acquired and before any canonical change. This also applies to
no-op requests, preventing a stale writer from reporting success merely because
the requested state already exists. Conflicts roll back without allocating a
revision and are returned to the caller without an automatic retry.

TTY note tables need full-column widths, so rendering spools encoded rows to an
unnamed temporary file before replaying them. Redirected note tables do not
require alignment and stream directly. Full-note and change JSONL also stream
directly, holding only the current record in memory.

## Schema Ownership

The application schema is a flat compile-time manifest in a fixed order:

- `src/schema.rs` owns the application ID, schema version, and version table.
- `src/note/schema.rs` owns revision state, note and change tables, note FTS, triggers, and indexes.
- `src/storage/schema_engine.rs` initializes and validates the assembled
  manifest without knowing the note model.

Opening storage validates identity, version, required object definitions, and
allowed triggers before returning a configured connection. Foreign keys are
enabled for every connection. Read-write connections establish WAL and use a
bounded busy timeout; read-only commands can operate against a non-writable
database when SQLite can read the database and referenced WAL.

Initialization can adopt an empty SQLite database or atomically publish a new
temporary sibling. On Unix it requests private directory and database modes.
Ordinary commands never create storage or schema objects. Note FTS is derived
and rebuildable; tokenizer changes are explicit schema changes. Retrieval
remains deterministic and non-scored.
