# nt Design

This document specifies the note model and application interface. Layer and
dependency rules are documented in `architecture.md`.

`nt` is a local, agent-first note application for editable CommonMark notes,
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
The home-directory environment path must be non-empty and absolute so canonical
storage cannot resolve relative to the working directory.

There are no note kinds, todos, agenda behavior, vaults, collection entities,
many-to-many memberships, sources, generic metadata, configurable projections,
reserved tags, automatic routing, or migration compatibility. Append-only
memory and other distinct product models are outside `nt` and belong in separate
applications.

External resources, bookmarks, imported documents, and agent-generated
summaries can be captured as ordinary CommonMark notes and organized with the
same collections, tags, and directional links. They have no reserved note kind,
source metadata, or hidden semantics.

## Initialization

`nt init` is the only command allowed to create `$HOME/.nt`, the database file,
or schema objects. The clean-sheet database uses application ID `0x4e544e54`
(`NTNT`) and schema version `1`.

Initialization accepts a missing database, a zero-length file, or an empty
SQLite database. A valid existing nt database is reported as already
initialized. A database with unrelated objects or another application ID is
rejected without modification. An incompatible nt schema is rejected with an
instruction to delete the development database. Identity and schema-shape
mismatches are distinct from corrupt or malformed database images, which receive
a stable corruption error. Busy or locked databases remain retryable, and other
inspection failures retain their underlying database diagnostics.

On Unix, missing storage directories are requested with mode `0700`, and new or
adopted empty database files are set to `0600` before note state is written.
Existing directory modes and valid initialized database modes are preserved.
New SQLite WAL and shared-memory files inherit the database mode. Other
platforms retain their native filesystem permission behavior.

For a missing database, initialization builds a temporary sibling and publishes
it without replacing a file created concurrently. The candidate must enter WAL
mode before publication, and failed candidates are removed.
For supported schema version `1`, every required table, virtual table, trigger,
and index must retain its canonical definition. Additional user-defined tables,
views, and indexes are tolerated, but they are not part of nt state or supported
for writes. Unknown triggers are rejected because they can alter nt mutations.

Ordinary commands validate existence, application identity, schema version, and
required schema definitions without creating files or objects. Recognized
operational connections enable foreign keys and apply a bounded busy timeout.
Retrieval commands open the database read-only; initialization and mutation
commands open read-write and establish WAL. Mutations use short transactions,
and no transaction remains open while reading stdin or waiting for
`$VISUAL`/`$EDITOR`.

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
    CHECK(length(id) = 36
          AND substr(id, 9, 1) = '-'
          AND substr(id, 14, 1) = '-'
          AND substr(id, 15, 1) = '7'
          AND substr(id, 19, 1) = '-'
          AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
          AND substr(id, 24, 1) = '-'
          AND length(replace(id, '-', '')) = 32
          AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(collection) > 0
          AND collection NOT GLOB '*[^a-z0-9_/-]*'
          AND substr(collection, 1, 1) <> '/'
          AND substr(collection, -1, 1) <> '/'
          AND instr(collection, '//') = 0),
    CHECK(length(body) > 0),
    CHECK(length(title) > 0),
    CHECK(created GLOB
          '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
    CHECK(updated GLOB
          '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
    CHECK(body_version > 0)
);

CREATE TABLE note_tags (
    note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY(note_pk, tag),
    CHECK(length(tag) > 0 AND tag NOT GLOB '*[^a-z0-9_-]*')
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
and target-link lookup. Cheap schema checks enforce canonical UUIDv7, collection,
tag, and timestamp shapes. Full note validation remains in Rust.

External-content FTS5 state is maintained by isolated SQLite triggers. Inserts
add an FTS row, body/title changes replace it, and deletion removes it.
Metadata-only updates do not rewrite FTS state.

## Note Model And Repository

Note types own UUIDv7 note identity, collection and tag validation, body
normalization, H1 validation and title extraction, deduplication, self-link
rejection, and body replacement. Invalid note state must not be constructible
through validated note constructors and methods. Validated note values do not
parse CLI tokens, render output, open SQLite, issue SQL, or know about FTS.

Command handlers orchestrate narrow repository operations and do not issue SQL.
Existence and conflict checks happen inside each mutation transaction. Body
replacement uses an expected body version; only body changes increment that
version. Metadata mutations update `updated` only when canonical state changes.
A real `nt link` change updates only the source that owns the outgoing-link set.
Deleting a target updates surviving sources before foreign-key cascades remove
their outgoing edges; deleting a source does not update its outgoing targets.
Timestamps are UTC wall-clock values with one-second resolution and are not a
monotonic mutation sequence. Multiple changes may share `updated`, and system
clock adjustments may move it backward.

Body input is fully buffered before validation and has no application-level size
limit. Available memory and disk space are the operational bounds.

`Repository` remains a concrete facade rather than a trait hierarchy. Note
storage and rehydration, relationship mutations, summary projection, and query
SQL compilation are kept in separate repository modules. Process-global storage
resolution, standard streams and terminal detection, and editor environment and
launching belong to the binary adapter. Command dispatch receives those
dependencies through one concrete application context so command behavior can
be tested without mutating process state.

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
the distinct metadata values currently referenced by notes. Redirected summary
output is streamed and treats a closed downstream pipe as successful early
termination.

## Retrieval

Structured filters and lexical terms are parsed into one query model and
compiled to bound SQL parameters. `list` accepts only structured filters;
`find` adds literal lexical terms. Expressions are AND-combined and ordered by
`updated DESC, id DESC`. Note retrieval is complete by default. An optional,
strictly positive `limit:` applies an explicit SQL result bound.

Directional link filters operate on one edge only and describe the returned
notes. `links-to:<target>` selects notes that point to the target, while
`linked-from:<source>` selects notes pointed to by the source. Both compile
through the shared query model to SQL over the integer note primary keys and
`note_links`; links are not loaded into Rust for filtering. A canonical source
ID absent from `notes` has normal query semantics and returns no matches.
Summaries retain only the outgoing-link count; incoming-link metadata is not
projected into every result row.

Each summary query returns the note fields, its complete tag set, and outgoing
link count in one SQLite row without selecting the body. Redirected commands
consume and encode those rows incrementally, so Rust memory does not grow with
the total match count and output errors stop the active query. SQLite performs
indexed correlated metadata lookups within the single statement, avoiding
application-level N+1 queries. TTY output spools rendered rows to an unnamed
temporary file while computing column widths, then replays them with full-table
alignment. Memory remains bounded by one summary row and I/O buffers; temporary
disk use grows with rendered output size.

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

Live backups must use SQLite's backup facilities. Copying only the main database
while connections are active is unsafe because committed state may remain in the
WAL file.

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
