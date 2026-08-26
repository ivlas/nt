# nt CLI Reference

This document is the public command, input, and output contract. For guided
examples, see [Using nt](usage.md). Product invariants and implementation
boundaries live in [Design](design.md) and [Architecture](architecture.md).

## Conventions

- Storage resolves below `HOME`, falling back to `USERPROFILE`, and is normally
  `$HOME/.nt/nt.sqlite3`.
- Note IDs are canonical lowercase UUIDv7 values. Exact-note commands require a
  full ID; `id:<prefix>` accepts a non-empty prefix of the canonical ID,
  including its hyphens where reached.
- Timestamps are UTC seconds such as `2026-08-22T14:30:12Z`.
- Angle brackets in syntax blocks denote values; they are not literal shell
  input.

## Commands

```text
nt init
nt add [metadata...] [-- body...]
nt show <id>
nt list [filter...]
nt list tags
nt list collections
nt read [filter...]
nt changes since:<revision>
nt find <term-or-filter...>
nt rm <id...>
nt edit <id> [if-rev:<revision>] [-- body...]
nt move <id> <collection> [if-rev:<revision>]
nt tag <id> <+tag|-tag> [if-rev:<revision>]
nt link <id> <+id|-id> [if-rev:<revision>]
nt help [command...]
```

Running `nt` is equivalent to `nt help`. The CLI is flagless: `-h`, `--help`,
`-V`, and `--version` are not aliases and are rejected.

Reference sections: [storage](#storage), [note input](#note-input),
[note queries](#note-queries), [note mutations](#note-mutations), and
[errors](#errors-and-operation).

## Storage

`nt init` is the only command that creates the storage directory, database, or
schema. It prints `initialized` for a new database and `already initialized`
for a valid existing database. The resolved home path must be non-empty and
absolute.

Ordinary commands never create or repair storage. They require application ID
`0x4e544e54`, schema version `6`, and the expected schema definitions. This
alpha version does not migrate older databases in place.

## Note Input

`add` accepts metadata before `--`:

```text
collection:<path>
tag:<tag,...>
link:<id,...>
```

A note has exactly one collection, defaulting to `inbox`. Collection segments
and tags use lowercase `a-z`, `0-9`, `_`, and `-`; collections separate segments
with `/`. Repeated tags and links are deduplicated. Unknown metadata fields are
errors. Every linked target must already exist.

The body comes from exactly one source: non-empty arguments after `--`, piped
stdin, or an editor. Trailing arguments are joined with one ASCII space. Empty
input and conflicting non-empty stdin plus trailing text are errors.

Editor input uses the first non-empty value of `VISUAL`, then `EDITOR`. The
value is parsed into an executable and arguments without invoking a shell.

CRLF and CR line endings are normalized to LF. The first line must be `# `
followed by non-whitespace title text, with no leading blank line. Other content
is preserved. Success prints `saved <id>`.

`edit <id>` replaces the complete body using the same sources and body rules.
An editor update is rejected if another body edit commits first. Supplying
`if-rev:<revision>` additionally rejects the edit if any canonical mutation
changed the note since that revision was observed. Success prints `updated
<id>`.

## Note Queries

`show <id>` writes the exact canonical body with no wrapper or added newline.

`list` and `read` accept these structured expressions; `find` accepts the same
expressions plus one or more lexical terms:

```text
id:<prefix>
collection:<path>
tag:<tag>
links-to:<id>
linked-from:<id>
created-since:<timestamp>
updated-since:<timestamp>
not:<filter>
limit:<positive-integer>
```

Expressions are combined with AND. Timestamp bounds are inclusive. `not:` wraps
one structured filter. `limit:` cannot repeat or be negated; without it, every
match is returned. Unknown lowercase field expressions and malformed values are
errors.

For `read`, repeated `id:<id>` expressions containing full canonical IDs form
one arbitrary-ID batch and are combined with OR before any other filters are
applied. ID prefixes retain the normal AND behavior. Requested IDs are
deduplicated, missing IDs are omitted without error, and output follows the
canonical result order rather than caller ID order. `limit:` applies after that
ordering.

Command-line length is platform-dependent, so large arbitrary-ID batches can
use `id:-` and provide one full canonical ID per stdin line:

```sh
nt list tag:rust | cut -f1 | jq -r . | nt read id:-
```

`id:-` can appear once and composes with explicit IDs and other filters. Empty
stdin, blank lines, and malformed IDs are errors. The stdin form has the same
deduplication, missing-ID, ordering, and limit semantics as argument IDs.

`links-to:<target>` returns notes that point to the target.
`linked-from:<source>` returns notes pointed to by the source. Both require full
note IDs and inspect only direct edges.

Lexical terms search note titles and bodies. They are split into Unicode
letter-or-digit tokens, deduplicated, and matched as full-text search (FTS)
literals. Matching uses complete tokens, case folding, and SQLite's supported
Latin-diacritic removal. There is no raw FTS syntax, prefix expansion, ranking,
fuzzy matching, or semantic search.

Results are ordered by `updated DESC, id DESC`. Timestamps have one-second
resolution, so ID ordering resolves ties but does not imply mutation order.

### Note Output

Note result columns are:

```text
id    updated    collection    title    tags    outgoing
```

`outgoing` counts links from the note. On redirected stdout, output is
headerless TSV. Text cells are JSON strings, tags are a sorted JSON array, and
the count is a JSON number:

```text
"<id>"\t"<updated>"\t"<collection>"\t"<title>"\t["<tag>"]\t<outgoing>
```

Each note occupies one physical line. TTY output adds an aligned header, removes
JSON quoting, and visibly escapes title control characters. `list tags` and
`list collections` return distinct values in lexical order, with a TTY header
or one JSON string per redirected line.

Redirected note results stream as rows are read. A downstream pipe closing early
is successful; other output failures are errors.

### Full Note Output

`read` selects notes with the same filters, ordering, completeness, and
SQL-backed `limit:` behavior as `list`, but returns complete records. Redirected
stdout is UTF-8 JSONL with one object per physical line and fields in this stable
order:

```json
{"id":"<id>","created":"<created>","updated":"<updated>","revision":<revision>,"collection":"<collection>","title":"<title>","tags":["<tag>"],"links":["<target-id>"],"body":"<canonical CommonMark>"}
```

`tags` and outgoing `links` are sorted lexically. JSON escaping preserves
arbitrary body newlines and Unicode without splitting a record across physical
lines. `revision` is the note's latest canonical mutation revision. Empty
results produce no bytes. TTY output presents labeled metadata
followed by the canonical body. `show` remains the exact-body primitive: unlike
`read`, it adds no record framing or metadata.

Complete note records are selected and hydrated set-wise and stream as they are
read. A downstream pipe closing early is successful; other output failures are
errors. Arbitrary-ID batches use one set-oriented statement and do not consume
one SQLite host parameter per ID. Use `id:-` when a large batch could exceed the
operating system's command-line length.

### Change Output

`changes since:<revision>` returns every canonical note change committed
strictly after the supplied nonnegative global revision. Revision zero reads the
complete retained feed. Results are ordered by `revision ASC, id ASC` and are
complete; timestamps are never cursors.

Redirected stdout is UTF-8 JSONL with one change per physical line and fields in
this stable order:

```json
{"revision":<revision>,"operation":"<operation>","id":"<note-id>"}
```

Operations are `add`, `edit`, `metadata`, and `remove`. `metadata` covers
collection, tag, and directional-link changes, including outgoing links removed
because a target was deleted. Deleted IDs remain in the feed. The feed stores no
historical body or complete note snapshot; use `read` or `show` for current live
content. Empty results produce no bytes. TTY output is an aligned table.

Rows stream directly from an index ordered by revision and ID. A downstream
pipe closing early is successful. A revision can contain several rows, so a
synchronizer must finish all rows for a revision before durably advancing its
cursor; after interruption it can safely replay from its last completed
revision.

## Note Mutations

```text
nt move <id> <collection> [if-rev:<revision>]    -> moved <id> <collection>
nt tag <id> <+tag|-tag> [if-rev:<revision>]      -> tagged <id> <operation>
nt link <id> <+id|-id> [if-rev:<revision>]       -> linked <id> <operation>
nt rm <id...>                                      -> removed <count>
nt move id:- <collection>                          -> moved <count> <collection>
nt tag id:- <+tag|-tag>                            -> tagged <count> <operation>
nt rm id:-                                         -> removed <count>
```

These changes are transactional. Adding an existing set value or removing a
missing one succeeds without changing `updated`. Links are directional;
self-links are errors, and a real link change updates only its source note. Link
addition requires an existing target. Removing a valid target ID that is not
stored succeeds as a no-op.

Single-note `edit`, `move`, `tag`, and `link` accept an optional positive
`if-rev:<revision>` precondition. The revision is the whole-note `revision`
returned by `read`, not a timestamp or change-feed cursor guess. The mutation
proceeds only when the live note still has exactly that revision. The check and
mutation share one immediate transaction: a stale mutation cannot partially
apply, is never silently retried, and exits with retryable status `4`. A no-op
with a matching revision remains a no-op; a stale request conflicts even when
its requested state already matches. A note deleted before the check is a
normal missing-note error.

`id:-` cannot be combined with `if-rev:` because one precondition cannot guard
notes with different revisions. Per-note conditional batches are not supported.

`rm` rejects duplicate IDs and validates every ID before deleting any note.
Deleting a target updates surviving source notes whose outgoing links are
removed. Deleting a source does not update its targets.

For `move`, `tag`, and `rm`, `id:-` replaces the positional note ID or complete
`rm` ID list and reads one canonical note ID per stdin line. Line endings may be
LF or CRLF. Empty stdin, blank lines, malformed IDs, duplicate IDs, and missing
notes are errors. Input is completely read and syntactically validated before a
write transaction is opened; every referenced note is then validated before
any canonical mutation. A failure changes nothing.

One batch applies one collection, one tag operation, or removal to the complete
input set in one SQLite transaction. Tag and move notes already satisfying the
request remain untouched. If other notes change, only those notes receive the
batch revision and `metadata` rows. An all-no-op tag or move batch allocates no
revision. Removal emits `remove` for each deleted note and one deduplicated
`metadata` row for every surviving source whose outgoing links changed, all at
the deletion revision. Batch success counts report requested IDs.

The database keeps one global integer revision, initialized to zero. Every real
successful mutation increments it exactly once and stamps each affected
surviving note with that revision. Multi-note deletion receives one revision.
No-op, failed, and rolled-back mutations do not produce an observable revision.
Revisions are allocated in the same immediate SQLite transaction as the change,
so they are unique, strictly increasing, persistent across process restarts, and
ordered by committed writer mutations rather than timestamps or UUIDs. The
compact change feed transactionally records affected IDs and operation classes,
including removals, without retaining note versions or historical bodies.

## Errors And Operation

Operational errors use `error: <message>` on stderr. TTY errors may be colored;
`NO_COLOR`, `TERM=dumb`, or redirected stderr disables color.

Stdout is contractual command data and remains deterministic. Stderr is for
actionable errors. Normal commands intentionally do not emit logs or tracing;
any future optional diagnostics must not change normal stdout or stderr.

| Exit | Meaning |
| ---: | --- |
| `0` | Success, including an intentionally closed streaming pipe |
| `1` | Other operational failure |
| `2` | Invalid command syntax, query, or value |
| `3` | Missing storage or note |
| `4` | Retryable database contention or mutation conflict |

Common stable messages include `run nt init first`, `database is not an nt
database`, `database is corrupt`, `database is busy; retry`, `note not found:
<id>`, `note changed while editing: <id>`, and `note revision conflict: <id>
(expected <revision>, found <revision>); retry`. Clap owns command-grammar
diagnostics.

Database commits and success-output writes cannot be atomic. If output fails
after commit, `nt` reports that the operation committed but acknowledgment
failed. Retrying `add` blindly can create a duplicate.

Read commands require readable storage; mutations require writable storage.
Connection and transaction mechanics are described in
[Architecture](architecture.md#command-lifecycle).
