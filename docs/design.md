# nt Design

This document specifies the separate note and memory models and their shared
application interface. Layer and dependency rules are documented in
`architecture.md`.

`nt` is a local, agent-first application for editable CommonMark notes,
immutable persistent memory, deterministic metadata, and lexical retrieval.
The previous vault, todo, membership, source, projection, and generic metadata
model is obsolete and has no compatibility requirement.

## Product Boundary

A note has one canonical body, one derived title, exactly one collection,
optional tags, optional directional links, creation and update timestamps, and a
body version used only for optimistic body-edit conflicts.

Memory is a separate first-class model. It stores canonical immutable raw
experience in one monotonically ordered append-only sequence. SQLite assigns
positive integer sequence numbers as public memory identities. Summaries,
summary jobs, and FTS indexes are derived state and do not alter raw history.

SQLite at `$HOME/.nt/nt.sqlite3` is canonical. Markdown exists at the note
interface boundary; there is no canonical Markdown filesystem or note rebuild
workflow. Public note IDs are lowercase UUIDv7 values. Internal integer keys
may be used for joins and FTS row IDs.
The home-directory environment path must be non-empty and absolute so canonical
storage cannot resolve relative to the working directory.

There are no note kinds, todos, agenda behavior, vaults, collection entities,
many-to-many memberships, sources, generic metadata, configurable projections,
reserved tags, automatic routing, or general migration compatibility. Memory
is not represented as a note kind, collection, tag, generic metadata, or hidden
note semantics. Notes and memory retain separate schemas and concrete
repositories.

External resources, bookmarks, imported documents, and agent-generated
reference summaries can be captured as ordinary CommonMark notes and organized
with the same collections, tags, and directional links. They have no reserved
note kind, source metadata, or hidden semantics.

## Initialization

`nt init` is the only command allowed to create `$HOME/.nt`, the database file,
or schema objects. The clean-sheet database uses application ID `0x4e544e54`
(`NTNT`) and schema version `2`.

Initialization accepts a missing database, a zero-length file, or an empty
SQLite database. A valid existing nt database is reported as already
initialized. A database with unrelated objects or another application ID is
rejected without modification. An incompatible nt schema is rejected with an
instruction to delete the development database. Identity and schema-shape
mismatches are distinct from corrupt or malformed database images, which receive
a stable corruption error. Busy or locked databases remain retryable, and other
inspection failures retain their underlying database diagnostics.

On Unix, missing storage directories are requested with mode `0700`, and new or
adopted empty database files are set to `0600` before application state is
written.
Existing directory modes and valid initialized database modes are preserved.
New SQLite WAL and shared-memory files inherit the database mode. Other
platforms retain their native filesystem permission behavior.

For a missing database, initialization builds a temporary sibling and publishes
it without replacing a file created concurrently. The candidate must enter WAL
mode before publication, and failed candidates are removed.
For supported schema version `2`, every required table, virtual table, trigger,
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

The application schema layer owns initialization, opening, and validation of the
whole nt database. Its flat compile-time manifest contains application, note,
and memory schema contributions. Storage implements model-agnostic SQLite
mechanics. Separate concrete note and memory repositories receive already-open
connections and own only their model's persistence; neither owns the
application database lifecycle.

Version 2 follows a clean-sheet alpha policy rather than a general migration
system. Older development databases are not upgraded in place. An incompatible
nt schema is rejected with an instruction to delete the database and run
`nt init`.

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

The application owns the `schema_version` table. The note and memory modules
contribute their model objects to one flat compile-time manifest:

```sql
CREATE TABLE schema_version (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    version INTEGER NOT NULL CHECK (version = 2)
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

CREATE TABLE memories (
    seq INTEGER PRIMARY KEY CHECK (seq > 0),
    body TEXT NOT NULL,
    created TEXT NOT NULL,
    CHECK(length(body) > 0 AND length(body) <= 1024),
    CHECK(instr(body, char(0)) = 0),
    CHECK(created GLOB
          '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
);

CREATE TABLE memory_segments (
    pk INTEGER PRIMARY KEY CHECK (pk > 0),
    level INTEGER NOT NULL CHECK (level >= 0),
    block INTEGER NOT NULL CHECK (block >= 0),
    summary TEXT NOT NULL,
    created TEXT NOT NULL,
    UNIQUE(level, block),
    CHECK(length(summary) > 0 AND length(summary) <= 1024),
    CHECK(instr(summary, char(0)) = 0),
    CHECK(created GLOB
          '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
);

CREATE TABLE memory_summary_jobs (
    level INTEGER NOT NULL CHECK (level >= 0),
    block INTEGER NOT NULL CHECK (block >= 0),
    PRIMARY KEY(level, block)
) WITHOUT ROWID;

CREATE VIRTUAL TABLE memory_fts USING fts5(
    body,
    content = 'memories',
    content_rowid = 'seq',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE memory_segment_fts USING fts5(
    summary,
    content = 'memory_segments',
    content_rowid = 'pk',
    tokenize = 'unicode61 remove_diacritics 2'
);
```

Note indexes support created and updated ordering, collection filtering, tag
lookup, and target-link lookup. Memory indexes support created ordering for raw
and summarized history. Cheap schema checks enforce canonical UUIDv7,
collection, tag, timestamp, positive memory identity, immutable body, and
summary-node shapes. Full model validation remains in Rust.

External-content FTS5 state is maintained by isolated SQLite triggers. Inserts
add an FTS row, note body/title changes replace it, and deletion removes it.
Metadata-only note updates do not rewrite FTS state. Raw memory inserts add raw
FTS rows, summary inserts and deletes update summary FTS, and raw-memory update
and delete triggers abort those mutations.

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

## Memory

The memory architecture is:

```text
immutable raw experience -> 16-way summary pyramid -> indexed retrieval
-> 32 KiB context compiler -> progressive expansion -> exact original history
```

Raw memories are canonical and immutable. Each append receives a
SQLite-assigned positive integer sequence, and list order follows that sequence.
The schema rejects updates and deletes of raw rows. Invalidation deletes only
derived summaries and jobs; exact raw history remains available through `show`,
`list`, `recall`, and progressive `expand`.

Raw bodies normalize CRLF and CR line endings to LF, must be non-empty, and may
not contain NUL. Raw bodies and summaries are each limited to 1,024 Unicode
characters. Limits count Unicode scalar values through Rust's `char` count, not
UTF-8 bytes. These values, the 32,768-character context stdout budget, and
fanout 16 are fixed compile-time constants with no configuration.

Raw memory has no CommonMark H1 requirement and no tags, links, collection,
kind, source, or generic metadata. A timestamp is stored at append time, but the
SQLite sequence is the public identity and deterministic history order.

Summaries, pending-summary jobs, raw FTS, and summary FTS are derived and
rebuildable from raw history. The public CLI does not currently provide one
whole-database rebuild command. Summary regeneration is explicit: the calling
agent asks for pending work, summarizes the supplied children, and submits the
result. Appending does not perform or wait for summarization. There is no daemon,
background worker, automatic summarization call, or built-in model launcher.

Retrieval uses literal FTS, deterministic SQL ordering, and fixed allocation in
Rust. It makes no model call and does not require embeddings. Summary quality is
therefore limited by the calling agent's submitted text; exact raw evidence is
the authority.

## Memory Tree

A node is written `L<level>:<block>`, where level and block are canonical
non-negative decimal integers representable in SQLite. With fanout `F = 16`,
the inclusive raw range of node `(L, B)` is:

```text
span(L)  = 16^(L + 1)
start    = B * span(L) + 1
end      = (B + 1) * span(L)
```

For `L = 0`, the 16 children are raw sequences `start` through `end`. For
`L > 0`, the children are nodes `(L - 1, 16 * B)` through
`(L - 1, 16 * B + 15)`. The parent of `(L, B)` is
`(L + 1, floor(B / 16))`. Exponents, additions, multiplications, conversions,
and range bounds use checked arithmetic. Unrepresentable nodes and ranges are
rejected rather than wrapped.

A level-zero job becomes eligible only after all 16 exact raw children exist.
A higher-level job becomes eligible only after all 16 exact child summaries
exist. Completing children out of order is supported; submitting the sixteenth
child repairs or creates the parent job. Pending jobs are ordered by level then
block, both ascending. Appending, summary creation, job removal or creation,
and invalidation each use short transactions, so canonical and derived state do
not expose partially applied operations.

Creating a summary validates that the node is pending and that all 16 expected
children exist. Repeating an existing node with the exact stored summary is
idempotent. Submitting different text for an existing node is a conflict.
Successful creation stores the summary, removes its job, and makes its parent
eligible when that parent's 16 children are complete.

Expansion requires an existing summary and reveals exactly one level: 16 raw
rows for level zero or 16 child summaries for a higher level. Repeated expansion
therefore reaches exact original history. Invalidation removes the selected
summary and every stored ancestor that depends on it, removes corresponding
stale jobs, and requeues the selected node when its children remain complete.
It never removes raw memory.

## Interface

Core workflows are flagless, configless, positional, and shared by humans and
agents. Note capture uses exactly one non-empty body source: trailing arguments
after `--`, piped stdin, or `$VISUAL`/`$EDITOR`. Memory bodies and summaries use
only trailing arguments after `--` or stdin and never invoke an editor. Editor
values for notes are parsed into argv and executed directly without a shell.
Input is fully read and validated before a write transaction begins.

The command and output contract is specified in `cli-reference.md`. Note `list`
and `find` retrieve fixed metadata summaries rather than bodies; note `show`
retrieves one exact body. Memory `list` and `recall` include complete raw bodies,
and memory `show` retrieves one exact raw body.

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

## Memory Retrieval

`memory list` applies inclusive `since:<seq>` and `until:<seq>` bounds and an
optional positive SQL `limit:<n>`, then orders raw rows by `seq ASC`. `memory
recall` applies the same filters to literal-token raw FTS. Recall orders by
`seq ASC`, independent of term frequency and document length. User terms are
split into Unicode alphanumeric runs, sorted, deduplicated, quoted as FTS
literals, and AND-combined. FTS5 is only a complete-token lexical filter. There
is no raw FTS syntax, fuzzy match, relevance score, semantic ranking, or
embedding lookup.

The context compiler selects complete raw bodies and summaries under a fixed
32,768-Unicode-character stdout budget. The cost of each item includes its
header, timestamp or range, complete content, separators, and newlines. Every
candidate-producing SQL query has `LIMIT 256`; this bound applies independently
to each pool.

With no lexical terms, allocation is:

```text
recent raw budget = 32768 * 60 / 100 = 19660
broad summary budget = 32768 - recent raw budget = 13108
```

Recent raw candidates are ordered `seq DESC`. Broad summary candidates are
ordered `level DESC, block DESC`.

With lexical terms, allocation is:

```text
lexical raw budget = 32768 * 40 / 100 = 13107
recent raw budget = 32768 * 30 / 100 = 9830
lexical summary budget = 32768 - lexical raw budget - recent raw budget = 9831
```

FTS5 acts only as a lexical filter for both pools. Lexical raw candidates are
ordered by `seq DESC`, preferring newer exact evidence. Lexical summary
candidates are ordered by `level DESC, block DESC`, preferring coarser summaries
and then newer blocks. Integer rounding remainder goes to the last pool.

After the preferred pools run, remaining capacity is offered to the already
bounded recent-raw and broad-summary candidate sets with a 60/40 fallback split.
If either fallback pool is sparse or cannot fit another complete item, the other
may consume the residue. Raw fallback is considered before broad-summary
fallback when both can consume the final residue.

Candidates are considered in pool order. A candidate that does not fit its pool
or the total stdout budget is skipped, not truncated. Raw sequence identities
are deduplicated across raw pools. Any summary whose inclusive raw range overlaps
a selected raw item or selected summary is rejected, so exact raw evidence wins
and summary coverage does not overlap. Selected items are finally ordered
chronologically by raw range, with deterministic end and kind ties.

The 32,768-character budget is checked against the complete rendered document,
so `nt memory context` stdout never exceeds it. Storage size and context size are
independent; no stored item is truncated merely because history is large. The
fixed 256 candidates per pool mean context is deterministic and SQL-bounded but
not exhaustive over all stored history. Literal lexical matching can miss
synonyms, wording changes, and facts omitted or misstated by a caller-produced
summary.

## Consistency

Every mutation uses one short transaction. Foreign keys enforce note
relationship cleanup, and FTS changes atomically with their note, raw-memory,
or summary mutation. Memory append assigns a sequence and conditionally creates
its level-zero job in one immediate transaction. Summarization stores a segment,
removes its job, and repairs parent eligibility in one immediate transaction.
Invalidation removes dependent summaries and jobs and requeues eligible work in
one immediate transaction. Multi-note deletion rejects duplicate IDs and
validates every ID before deleting any row.

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
query syntax, body conflicts, idempotent metadata changes, link cleanup,
immutable memory, tree arithmetic, summary jobs and conflicts, out-of-order
completion, both FTS paths, invalidation, progressive expansion, concurrent
append, bounded deterministic context, busy handling, and output encoding.

Before release, run:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets
cargo run --locked -- help
```
