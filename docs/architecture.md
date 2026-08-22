# nt Architecture

`nt` is one local, agent-first application with two separate first-class
concrete models: editable CommonMark notes and immutable persistent memory. It
is one Cargo package with direct, compile-time dependencies rather than traits,
plugins, registries, or dependency-injection machinery.

## Dependency Direction

The primary command paths are:

```text
CLI -> commands -> application schema -> SQLite storage
                -> note repository   -> opened SQLite connection
                -> memory repository -> opened SQLite connection
```

`src/lib.rs` is the process composition root and `run_process` is the only
intentional public Rust entry point. It resolves process state, builds the
application context, and dispatches the parsed command. Note, memory,
repository, CLI, and schema APIs remain implementation details rather than a
supported SDK.

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
  memory/
    mod.rs
    model.rs
    query.rs
    schema.rs
    tree.rs
    repository/
      context.rs
      mod.rs
  storage/
  cli/
  commands/
    memory.rs
  app.rs
  schema.rs
  error.rs
  lib.rs
  main.rs
```

`src/error.rs` defines the concrete crate-wide error vocabulary, including the
stable process categories consumed by the binary adapter.

## Responsibilities

`src/cli/` owns command grammar, body input, editor execution for notes,
rendering, terminal behavior, help, and canonical home-directory resolution.
Memory bodies and summaries come only from trailing arguments after `--` or
stdin; memory commands never launch an editor.

`src/commands/` owns orchestration. Handlers parse concrete values, open the
appropriate connection, select a narrow note or memory repository operation,
and render the result without issuing SQL. `src/commands/memory.rs` owns the
memory command flow and its stable textual formats.

`src/note/` owns editable durable knowledge: UUIDv7 identity, body and metadata
validation, collections, tags, directional links, note queries, optimistic
body-edit conflicts, concrete persistence operations, and note schema SQL.

`src/memory/` owns immutable durable experience and history. It defines
SQLite-assigned sequence identities, raw and summary validation, literal query
parsing, checked tree arithmetic, concrete persistence operations, summary-job
state, deterministic context selection, and memory schema SQL. It does not use
the note schema or represent memory through reserved notes, collections, tags,
or metadata.

The note and memory `Repository` types are separate concrete facades over
already-open SQLite connections. Neither is a repository trait. Neither opens,
initializes, or validates the application database.

`src/storage/` owns SQLite infrastructure: opening and configuring connections,
foreign keys, WAL, busy handling, private filesystem setup, atomic publication
of newly initialized databases, and exact schema-validation mechanics. It does
not own note or memory models or their schema SQL.

`src/app.rs` carries the concrete, testable process dependencies used by command
handlers. `src/schema.rs` owns application database initialization and opening.
It binds the fixed nt database identity and flat application schema manifest to
the storage mechanics and returns configured, validated SQLite connections.

The production dependency rules are:

```text
storage does not import note or memory
note and memory do not import CLI, commands, or App
note and memory remain separate concrete models and repositories
application schema imports model schema definitions, not repositories
repositories operate only on connections opened by the application schema layer
commands construct the concrete repository needed for one operation
CLI parses and renders note-facing and memory-facing values
```

Tests may construct the complete application across these boundaries to verify
integration behavior.

## Memory Pipeline

The memory architecture is fixed in substance:

```text
immutable raw experience -> 16-way summary pyramid -> indexed retrieval
-> 32 KiB context compiler -> progressive expansion -> exact original history
```

Raw memory is the canonical, append-only history. SQLite assigns each raw entry
a positive, monotonically increasing public sequence number. Database triggers
reject updates and deletes. Raw entries survive summary invalidation and are the
evidence reached by repeated expansion.

Every complete group of 16 raw entries becomes eligible for a level-zero
summary. Every complete group of 16 summaries becomes eligible at the next
level. The calling agent obtains explicit work with `nt memory pending`,
compresses the provided 16 children, and submits the result with `nt memory
summarize`. Appending never waits for summarization. There is no daemon,
background worker, or built-in model call.

Raw-memory FTS, summary FTS, pending jobs, and summaries are derived,
rebuildable state. Summary text is supplied by the calling agent but can be
regenerated by traversing immutable children. Retrieval itself performs only
deterministic SQLite and Rust work. It requires no embeddings and makes no
model call.

The context compiler has a fixed 32,768-Unicode-character memory-content budget
and selects only complete raw bodies or summaries. Labels added to the rendered
document do not consume that budget. Storage size and context size are
independent: the append-only history may grow without changing the bounded
context allocation. `nt memory expand` reveals exactly one child level at a
time, allowing progressive recovery from a selected summary to exact original
history.

The memory v1 limits are compile-time constants with no configuration:

| Limit | Value |
| --- | ---: |
| Raw memory body | 1,024 Unicode characters |
| Summary body | 1,024 Unicode characters |
| Context memory content | at most 32,768 Unicode characters |
| Summary fanout | 16 |

## Schema Ownership

`src/schema.rs` owns application ID `0x4e544e54`, schema version `2`, and one
flat compile-time manifest. Its contributions are application schema objects,
note schema objects, and memory schema objects in a fixed order. There is no
schema-fragment registry, runtime registration, or plugin composition model.

The application contribution owns `schema_version`. `src/note/schema.rs` owns
note tables, note FTS, triggers, and indexes. `src/memory/schema.rs` owns raw
memories, memory summaries, summary jobs, both memory FTS indexes, immutability
and FTS triggers, and memory indexes. `src/storage/schema_engine.rs` consumes
the flat manifest to perform transactional initialization, identity inspection,
exact object validation, version checks, and unknown-trigger rejection.

Schema version 2 follows the clean-sheet alpha policy, not a general migration
system. An incompatible development database is rejected with an instruction
to delete it and run `nt init`; version 1 is not upgraded in place. Required
version-2 SQL definitions and creation order are compatibility boundaries for
the current alpha schema.

## Product Boundary

Notes and memory solve different persistence problems and stay separate. Notes
are editable durable knowledge with one collection, optional tags, and explicit
directional links. Memory is immutable durable experience in one append-only
sequence with derived summaries and indexes. Their schemas and repositories are
not merged into a generic entity or generic repository.

External resources, bookmarks, imported documents, and generated reference
summaries remain ordinary CommonMark notes organized by collections, tags, and
directional links. They receive no reserved note kinds or hidden semantics.
Persistent experience belongs to memory and is not encoded as a special note.
