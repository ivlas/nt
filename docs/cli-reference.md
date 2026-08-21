# nt CLI Reference

`nt` is a flagless, configless local CLI over `$HOME/.nt/nt.sqlite3`. SQLite is
canonical for authored notes and externally sourced Library evidence.

## Commands

```text
nt init
nt add [metadata...] [-- body...]
nt show <id>
nt list [filter...]
nt list tags
nt list collections
nt find <term-or-filter...>
nt rm <id...>
nt edit <id> [-- body...]
nt move <id> <collection>
nt tag <id> <+tag|-tag>
nt link <id> <+id|-id>
nt library add <source> <title...>
nt library capture <library-id>
nt library show <library-id>
nt library find <term-or-filter...>
nt library summary <library-id>
nt library history <library-id>
nt ref <note-id> <library-id>
nt unref <note-id> <library-id>
nt help [command...]
```

Running `nt` is equivalent to `nt help`. Exact-note commands require a full
canonical lowercase UUID. `id:<prefix>` filters accept a non-empty hexadecimal
UUID prefix and return every match.

## Initialization

`nt init` is the only command that creates `$HOME/.nt`, the database file, or
schema objects. It prints `initialized` for a new database and
`already initialized` for a valid existing database.
`HOME`, or the `USERPROFILE` fallback, must be a non-empty absolute path.

On Unix, missing storage directories are requested with mode `0700`; new and
adopted empty databases use mode `0600`. Existing directory modes and valid
initialized database modes are not changed. New WAL and shared-memory files
inherit the database mode. Other platforms use native permission behavior.

Ordinary commands do not create storage. They validate application ID
`0x4e544e54`, clean-sheet schema version `2`, and the exact definitions of every
required schema object before operating. Additional user-defined tables, views,
and indexes are tolerated but are outside nt's supported state. Unknown triggers
are rejected because they can alter nt writes.

## Capture

`add` accepts metadata only before `--`:

```text
collection:<path>
tag:<tag,...>
link:<id,...>
```

At most one collection is accepted and it defaults to `inbox`. Tags and links
may repeat and are deduplicated. Unknown metadata fields are errors. A
collection is a lowercase `/`-separated path whose segments contain only
`a-z`, `0-9`, `_`, and `-`. Tags use the same characters without `/`.

The body comes from exactly one source: non-empty arguments after `--`, piped
stdin, or an editor. Trailing arguments are joined with one ASCII space. Without
trailing arguments, empty redirected stdin and empty editor output are errors.
Non-empty stdin combined with trailing body text is an error; an empty redirect
such as `/dev/null` does not conflict with explicit trailing text.

Editor input uses the first non-empty value of `$VISUAL`, then `$EDITOR`. The
value is parsed as POSIX words into an executable and fixed arguments, and the
temporary note path is appended as one final argument. No shell is invoked, so
expansion, redirection, pipelines, and other shell operators are not evaluated.

The canonical body normalizes CRLF and CR line endings to LF. Its first line
must be `# Non-empty title`, with no leading blank line. Success prints
`saved <id>`.

```sh
nt add tag:rust,sqlite -- '# Storage'
printf '%s\n' '# Storage' '' 'SQLite is canonical.' | nt add tag:rust
nt add collection:work/nt
```

## Show And Listing

`show <id>` writes the exact canonical body with no metadata wrapper.

`list` accepts structured filters. `list tags` and `list collections` print the
distinct values currently used by notes, in lexical order. `find` accepts the
same filters and one or more lexical terms:

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

Expressions are AND-combined. `not:` wraps exactly one structured filter.
`limit:` controls result count rather than filtering notes and cannot be repeated
or negated. Without `limit:`, `list` and `find` return every matching note. An
explicit limit selects the first N results in normal order and is applied by
SQLite. Unknown fields, malformed filters, invalid limits, and negated lexical
terms are errors.

Directional link filters select opposite sides of the same stored edge:

```text
links-to:<target>      notes that point to target
linked-from:<source>   notes pointed to by source
```

Both require a full canonical lowercase UUIDv7 note ID. A valid source ID that
does not exist, or identifies a note with no outgoing links, produces zero
matches. Directional filtering remains non-recursive and does not add incoming
link metadata or link targets to summary columns.

Bare `find` arguments are split into Unicode letter-or-digit runs, deduplicated,
quoted as FTS literals, and AND-combined. Matching is complete-token,
case-folded, and Latin-diacritic-insensitive where supported by SQLite
`unicode61`. There is no raw FTS syntax, prefix expansion, ranking, scoring,
fuzzy matching, or metadata substring fallback.

Results are ordered by `updated DESC, id DESC`. Timestamps have one-second
resolution, so changes within one second can tie. Descending ID makes those ties
deterministic but does not represent their mutation order. Results use columns:

```text
id    updated    collection    title    tags    outgoing
```

`outgoing` is the number of directional links from the note. Redirected output
is headerless TSV. Text cells are JSON strings, tags are a lexicographically
sorted JSON array, and `outgoing` is a JSON number, preserving one physical line
per note:

```text
"<id>"\t"<updated>"\t"<collection>"\t"<title>"\t["<tag>"]\t<outgoing>
```

TTY note output removes JSON quoting, adds a header row, and aligns columns with
two spaces between them while preserving values and column order. Metadata
inventories use one `tag` or `collection` column with a TTY header. Redirected
inventory output is headerless and contains one JSON string per line. Redirected
note summaries are streamed as SQLite rows are read. A downstream pipe closing
early ends note retrieval successfully; other output errors remain failures.

## Mutations

`edit` replaces the complete body using the same input rules as `add`. An editor
session is rejected if another body edit changes the note first. Metadata
changes do not conflict with an open editor session.

```sh
nt move <id> work/nt
nt tag <id> +decision
nt tag <id> -decision
nt link <id> +01989abc-...
nt link <id> -01989abc-...
```

Moves, tag changes, and link changes are transactional. A real `nt link`
addition or removal updates the source note; the target is unchanged. Adding an
existing set value or removing a missing one succeeds without changing
`updated`. Self-links are errors. Links remain directional.

Success output is exact:

```text
updated <id>
moved <id> <collection>
tagged <id> <operation>
linked <id> <operation>
removed <count>
```

`rm` rejects duplicate IDs and validates every ID in one transaction before
deleting anything. Any missing or invalid ID leaves all requested notes intact.
Before foreign-key cascades remove incoming links, each surviving source note is
updated because its canonical outgoing-link set changed. Deleting a source does
not update its outgoing targets.

## Library

Library stores evidence; Notes store synthesis. `library add` reads exact
external content from stdin or `$VISUAL`/`$EDITOR`, creates a stable Library item
for a new source, and creates its first immutable capture. Resolving an existing
source preserves its original title and appends only content not already stored
for that item. Exact content bytes are deduplicated using BLAKE3.

`library capture` appends changed content without rewriting history. `library
show` prints only the latest exact capture. `library summary` replaces the
caller-supplied summary attached to the latest capture, using generator `manual`
and version `1`. A later capture has no summary until one is explicitly supplied.
`library history` lists capture timestamp, content hash, generator, version,
summary, and summary timestamp without printing captured content.

`library find` searches only the latest capture of each item and returns each
item at most once. Bare terms and `text:` values are literal Unicode FTS terms
with AND semantics. Supported filters are:

```text
id:<prefix>
source:<exact-source>
title:<case-insensitive-substring>
text:<literal-term>
since:<captured-timestamp>
before:<captured-timestamp>
limit:<positive-integer>
```

Redirected find rows are JSON-encoded, headerless TSV in this order:

```text
id  source  title  created  updated  captured  summary
```

Successful Library mutations print `library saved <id>`, `library captured
<id>`, `library unchanged <id>`, `captured <id>`, or `summarized <id>`.

`ref` and `unref` manage an explicit evidence edge from a note to a Library
item. They are separate from directional note `link` edges, require both targets
to exist, and are idempotent. Deleting either target cascades its references.

## Errors

Operational errors write `error: <message>` to stderr. Exit codes are `2` for
invalid syntax, queries, or values; `3` for missing storage or notes; `4` for
retryable database contention or concurrent edits; and `1` for other operational
failures. Success and an intentionally closed summary-output pipe return `0`.
Errors before success-output writing do not print to stdout. Stable messages are:

```text
error: run nt init first
error: home directory not found
error: database is not an nt database
error: database is corrupt
error: database could not enter WAL mode
error: system clock is outside the supported timestamp range
error: stored note is invalid (<safe note or row identity>, field: <field>)
error: unsupported nt schema version <version>; delete ~/.nt/nt.sqlite3 and run nt init
error: database is busy; retry
error: note not found: <id>
error: note changed while editing: <id>
error: invalid <field>: <value>
error: body is empty
error: body must begin with '# <title>'
error: cannot combine body arguments with stdin
error: VISUAL or EDITOR is not set
error: invalid VISUAL or EDITOR command
error: failed to launch editor: <io error>
error: editor exited unsuccessfully with status <status>
error: duplicate note id: <id>
error: cannot link note to itself
error: failed to <filesystem operation> `<path>`: <io error>
error: failed to open database `<path>`: <database error>
```

Database commits and success-output writes cannot be atomic. If success output
fails after a mutation commits, nt returns nonzero with `error: operation
committed but success output failed: <io error>`. The mutation remains committed,
stdout may contain a partial acknowledgment, and blindly retrying `add` can
create a duplicate note.

During SQLite inspection, `database is not an nt database` is reserved for
recognized application-identity or schema-shape mismatches. Corrupt or malformed
database images use the stable corruption message, and busy or locked databases
use the retryable busy message. Other unexpected SQLite failures retain
diagnostic detail rather than being misreported as foreign databases. Command
grammar errors use clap's stderr diagnostics and exit code `2`.

## Operation

Every mutation uses one short SQLite transaction. Foreign keys are enabled on
all operational connections. `show`, `list`, and `find` open the database in
SQLite read-only mode and work when the database file is not writable, provided
SQLite can read the database and any WAL state it references. `nt init` and
mutation commands require a writable database and establish WAL. WAL lets
readers continue from the last committed snapshot while another connection
writes. A contending writer waits for the bounded busy timeout and then reports
the stable retryable error. No transaction remains open while reading stdin or
waiting for `$VISUAL`/`$EDITOR`.
