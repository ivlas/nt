# nt Design

`nt` is a local, agent-first note layer for editable CommonMark notes,
deterministic metadata, and lexical retrieval. The previous vault, todo,
membership, source, projection, and generic metadata model is obsolete and has
no compatibility requirement.

## Product Boundary

A note has one canonical body, one derived title, exactly one collection,
optional tags, optional directional links, creation and update timestamps, and a
body version used only for optimistic body-edit conflicts.

SQLite at `$HOME/.nt/nt.sqlite3` is canonical. Markdown exists at the interface
boundary; there is no canonical Markdown filesystem or rebuild workflow. Public
note IDs are lowercase UUIDv7 values. Internal integer keys may be used for
joins and FTS row IDs.

There are no note kinds, todos, agenda behavior, vaults, collection entities,
many-to-many memberships, sources, generic metadata, configurable projections,
reserved tags, automatic routing, or migration compatibility. Append-only
memory requires a separate workflow-backed RFC and must remain an independent
subsystem.

## Initialization

`nt init` is the only command allowed to create `$HOME/.nt`, the database file,
or schema objects. The clean-sheet database uses application ID `0x4e544e54`
(`NTNT`) and schema version `1`.

Initialization accepts a missing database, a zero-length file, or an empty
SQLite database. A valid existing nt database is reported as already
initialized. A database with unrelated objects or another application ID is
rejected without modification. An incompatible nt schema is rejected with an
instruction to delete the development database.

Ordinary commands validate existence, application identity, and schema version
without creating files or objects. Recognized operational connections enable
foreign keys, use WAL, and apply a bounded busy timeout. Mutations use short
transactions, and no transaction remains open while reading stdin or waiting
for `$VISUAL`/`$EDITOR`.

## Notes

The canonical body is non-empty CommonMark with CRLF and CR line endings
normalized to LF. Other content is preserved. The first line must begin with
`# ` and have non-whitespace content after it. There is no leading blank line or
Setext-title support. The trimmed remainder of the first line is the title.

A collection is a lowercase ASCII path with one or more `/`-separated segments.
Segments contain only `a-z`, `0-9`, `_`, and `-`; empty segments and leading or
trailing slashes are invalid. Every note has one collection, defaulting to
`inbox`.

Tags use lowercase ASCII `a-z`, `0-9`, `_`, and `-`. Links are directional:
linking A to B creates only A to B. Duplicate tags and links are eliminated,
self-links are rejected, and deleting a note removes its tags and incoming and
outgoing links through foreign-key cascades.

## Schema

Primary state uses these tables:

```sql
CREATE TABLE schema_version (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    version INTEGER NOT NULL CHECK (version = 1)
) WITHOUT ROWID;

CREATE TABLE notes (
    pk INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    collection TEXT NOT NULL,
    body TEXT NOT NULL,
    title TEXT NOT NULL,
    created TEXT NOT NULL,
    updated TEXT NOT NULL,
    body_version INTEGER NOT NULL DEFAULT 1,
    CHECK(length(collection) > 0),
    CHECK(length(body) > 0),
    CHECK(length(title) > 0),
    CHECK(length(created) = 20),
    CHECK(length(updated) = 20),
    CHECK(body_version > 0)
);

CREATE TABLE note_tags (
    note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY(note_pk, tag)
);

CREATE TABLE note_links (
    note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
    target_note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
    PRIMARY KEY(note_pk, target_note_pk),
    CHECK(note_pk <> target_note_pk)
);

CREATE VIRTUAL TABLE note_fts USING fts5(
    title,
    body,
    content = 'notes',
    content_rowid = 'pk',
    tokenize = 'unicode61 remove_diacritics 2'
);
```

Indexes support created and updated ordering, collection filtering, tag lookup,
and target-link lookup. Timestamps are fixed-width UTC RFC 3339 seconds.

External-content FTS5 state is maintained by isolated SQLite triggers. Inserts
add an FTS row, body/title changes replace it, and deletion removes it.
Metadata-only updates do not rewrite FTS state.

## Domain And Repository

Domain types own UUIDv7 note identity, collection and tag validation, body
normalization, H1 validation and title extraction, deduplication, self-link
rejection, and body replacement. Invalid note state must not be constructible
through public domain APIs. Domain code does not parse CLI tokens, render output,
open SQLite, or know about FTS.

Command handlers orchestrate narrow repository operations and do not issue SQL.
Existence and conflict checks happen inside each mutation transaction. Body
replacement uses an expected body version; only body changes increment that
version. Metadata mutations update `updated` only when canonical state changes.

## Interface

Core workflows are flagless, configless, positional, and shared by humans and
agents. Capture uses exactly one non-empty body source: trailing arguments after
`--`, piped stdin, or `$VISUAL`/`$EDITOR`. Editor values are parsed into argv and
executed directly without a shell. Input is fully read and validated before a
write transaction begins.

The command and output contract is specified in `cli-reference.md`. `list` and
`find` retrieve fixed metadata summaries rather than bodies. `show` retrieves one
exact body.

Redirected list and find output is headerless JSON-encoded TSV with one physical
line per note. Summaries include the outgoing-link count without loading link
targets. TTY output adds aligned headers and removes JSON quoting without
changing values or column order. `list tags` and `list collections` enumerate
the distinct metadata values currently referenced by notes.

## Retrieval

Structured filters and lexical terms are parsed into one query model and
compiled to bound SQL parameters. `list` accepts only structured filters;
`find` adds literal lexical terms. Expressions are AND-combined and ordered by
`updated DESC, id DESC`.

Users cannot submit raw FTS5 syntax. Lexical input is split into Unicode
letter-or-digit runs, deduplicated, quoted as literals, and AND-combined.
Matching uses complete tokens without prefix expansion, ranking, scoring, fuzzy
matching, or metadata substring fallback. Candidate retrieval does not load
bodies into Rust.

## Consistency

Every mutation uses one short transaction. Foreign keys enforce relationship
cleanup, and FTS changes atomically with canonical note content. Multi-note
deletion rejects duplicate IDs and validates every ID before deleting any row.

WAL permits readers to continue from the last committed snapshot while another
connection writes. SQLite remains single-writer; contention waits for the busy
timeout and then returns a stable retryable error.

## Development

Existing development databases may be deleted. Obsolete code should be removed
as its replacement lands rather than retained behind compatibility paths.

Required verification covers database identity, schema constraints, UUIDv7 IDs,
body validation, default collection behavior, transactions, command routing,
query syntax, body conflicts, idempotent metadata changes, link cleanup, FTS
synchronization, busy handling, and output encoding.

Before release, run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets
cargo run -- help
```
