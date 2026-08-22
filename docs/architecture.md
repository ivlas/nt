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
                -> memory repository -> opened SQLite connection
```

`src/lib.rs` is the process entry point. `run_process` resolves process state,
builds the concrete application context, and dispatches a parsed command. Other
Rust APIs are implementation details, not a supported SDK.

Production dependencies follow these rules:

```text
storage does not import note or memory
note and memory do not import CLI, commands, or App
note and memory remain separate concrete models and repositories
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
| `src/memory/` | Raw and summary values, tree arithmetic, queries, context selection, persistence, jobs, and memory schema SQL |
| `src/storage/` | SQLite connections, foreign keys, WAL, busy handling, filesystem setup, atomic initialization, and schema validation mechanics |
| `src/schema.rs` | Application identity, complete schema manifest, initialization, and validated read-only or read-write opening |
| `src/app.rs` | Concrete process dependencies used to test command handlers without mutating global process state |
| `src/error.rs` | Crate-wide errors and stable process exit categories |

Command handlers do not issue SQL. The note and memory `Repository` types are
separate concrete interfaces over already-open connections. They do not
resolve, initialize, open, or validate the application database.

Note and memory stay separate at the schema and repository layers. Features
that compose them belong in command or read-time application logic, not schema
coupling or a generic entity model.

## Command Lifecycle

Read commands open a validated read-only connection, execute repository
operations, and render the result. Exact `show` commands write and flush the
canonical body. Redirected note results and raw-memory list or recall results
stream rows; an intentionally closed downstream pipe ends those commands
successfully.

Multi-query reads may use a read transaction when they need one consistent
snapshot. Input-taking mutations collect and validate their body outside a
database connection where possible. Note editing reads the current body and
version, closes storage while the editor runs, then reopens storage and commits
only if the body version still matches. No transaction or connection remains
open while waiting for editor input.

Each mutation performs one short repository transaction. The commit completes
before its success line is written, so a later output failure cannot roll back
the mutation. This boundary is surfaced explicitly by the CLI error contract.

TTY note tables need full-column widths, so rendering spools encoded rows to an
unnamed temporary file before replaying them. Redirected note tables and memory
list, recall, or pending rows do not require alignment and stream directly.

## Schema Ownership

The application schema is a flat compile-time manifest in a fixed order:

- `src/schema.rs` owns the application ID, schema version, and version table.
- `src/note/schema.rs` owns note tables, note FTS, triggers, and indexes.
- `src/memory/schema.rs` owns raw memories, summaries, jobs, both memory FTS
  indexes, immutability triggers, FTS triggers, and memory indexes.
- `src/storage/schema_engine.rs` initializes and validates the assembled
  manifest without knowing the note or memory models.

Opening storage validates identity, version, required object definitions, and
allowed triggers before returning a configured connection. Foreign keys are
enabled for every connection. Read-write connections establish WAL and use a
bounded busy timeout; read-only commands can operate against a non-writable
database when SQLite can read the database and referenced WAL.

Initialization can adopt an empty SQLite database or atomically publish a new
temporary sibling. On Unix it requests private directory and database modes.
Ordinary commands never create storage or schema objects.

Memory FTS indexes are derived and rebuildable. They use SQLite FTS5 Porter
stemming over Unicode61 for primarily English technical memory; tokenizer
changes are explicit schema changes. Retrieval remains deterministic and
non-scored.
