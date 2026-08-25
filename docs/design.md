# nt Design

This document records product invariants and retrieval policy. The public
command contract is in [CLI Reference](cli-reference.md); code ownership is in
[Architecture](architecture.md).

## Product Model

`nt` is a local, agent-first application for editable durable knowledge as
CommonMark documents with one collection, optional tags, and directional links.

SQLite in the resolved home directory is canonical. There is no canonical
Markdown vault, JSON index, or rebuild-from-files workflow.

There are no note kinds, todos, collection entities, additional memberships,
sources, generic metadata, reserved tags, or hidden agent-only semantics.
Bookmarks, imports, external resources, and generated reference documents are
ordinary notes.

## Canonical And Derived State

| Model | Canonical state | Derived state |
| --- | --- | --- |
| Notes | body, collection, tags, links, timestamps, body version, note revision | title, note FTS |
| Database | global revision, compact note change feed | none |

## Notes

Public note IDs are canonical lowercase UUIDv7 values. A canonical body is
non-empty CommonMark with CRLF and CR normalized to LF. Its first line begins
with `# ` and contains a non-whitespace title. The trimmed remainder of that
line is stored as the title; other body content is preserved.

Every note has exactly one collection, defaulting to `inbox`. A collection is a
lowercase path with `/`-separated segments; tags use the same lowercase
characters without `/`. Tags and links are sets. Links are explicit,
directional, and cannot target the source note itself.

Body edits use an expected body version so two editor sessions cannot silently
overwrite each other. Metadata changes do not conflict with an open editor.
No-op set changes preserve `updated`; real metadata changes update it. UTC
timestamps have one-second resolution and are not a monotonic mutation order.

The global revision starts at zero and is a durable, monotonically increasing
SQLite integer. Every committed application mutation that changes canonical
note state increments it exactly once in the same transaction. A multi-note
mutation still receives one revision. No-op mutations do not increment it;
failed and rolled-back allocations are never observable. Because mutations use
`BEGIN IMMEDIATE`, SQLite serializes writers before allocation, so committed
revision order is committed mutation order across threads and processes.

Every live note stores the revision of the latest mutation that changed its
canonical state. All surviving notes changed by one mutation receive the same
revision. Deletion increments the global revision even when no note survives;
surviving sources whose links are removed receive the deletion revision. A
compact change feed records each affected note ID and whether the mutation was
an add, body edit, metadata change, or removal. It retains deleted IDs but no
historical bodies or complete note versions. Its revision-and-ID primary key
supports ordered incremental traversal. Revisions are not derived from
timestamps, UUIDs, or process-local state.

Multi-note deletion is atomic. Deleting a target removes incoming edges and
updates surviving sources because their outgoing-link sets changed. Deleting a
source does not update its targets.

## Retrieval

Retrieval is deterministic and lexical. User text is tokenized into literal
full-text search (FTS) terms; there is no scoring, fuzzy matching, raw FTS
syntax, embedding search, or automatic model call. SQL applies explicit limits
and stable ordering.

Note `list` and `find` return metadata rows without bodies. `read` applies the
same structured filters as `list` and streams complete note records, including
canonical bodies, without per-note retrieval. These commands are complete by
default. Literal search can miss alternate wording, so callers should inspect
exact note bodies when evidence matters.

Repeated full `id:` expressions on `read` select an arbitrary deduplicated ID
set. Missing IDs are omitted and canonical query ordering is preserved. Batch
size does not alter these semantics or cause per-note database operations.
`id:-` accepts the same exact-ID set as newline-delimited stdin when the set is
too large for platform command-line limits.

`changes` streams canonical invalidations strictly after a supplied global
revision in ascending revision and ID order. Multiple rows may share one
revision, so consumers advance a durable cursor only after processing the whole
revision. Retrying from the previous completed revision is deterministic.
Compaction would require an explicit retained-history boundary in this contract;
no automatic compaction is currently performed.

## Storage And Consistency

Only `nt init` creates storage. The database uses application ID `0x4e544e54`
(`NTNT`) and clean-sheet schema version `6`. This alpha policy rejects
incompatible databases instead of migrating them in place. Every mutation is
transactional, including its relationship and full-text index changes.

Schema ownership, connection lifecycles, and transaction boundaries are
described in [Architecture](architecture.md). Documentation intentionally does
not duplicate schema SQL.
