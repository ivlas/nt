# nt CLI Reference

`nt` is a flagless, configless local CLI over `$HOME/.nt/nt.sqlite3`. SQLite is
canonical for note bodies and metadata.

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
nt help [command...]
```

Running `nt` is equivalent to `nt help`. Exact-note commands require a full
canonical lowercase UUID. `id:<prefix>` filters accept a non-empty hexadecimal
UUID prefix and return every match.

## Initialization

`nt init` is the only command that creates `$HOME/.nt`, the database file, or
schema objects. It prints `initialized` for a new database and
`already initialized` for a valid existing database.

Ordinary commands do not create storage. They validate application ID
`0x4e544e54` and clean-sheet schema version `1` before operating.

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
link:<id>
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

Bare `find` arguments are split into Unicode letter-or-digit runs, deduplicated,
quoted as FTS literals, and AND-combined. Matching is complete-token,
case-folded, and Latin-diacritic-insensitive where supported by SQLite
`unicode61`. There is no raw FTS syntax, prefix expansion, ranking, scoring,
fuzzy matching, or metadata substring fallback.

Results are ordered by `updated DESC, id DESC` and use columns:

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

Moves, tag changes, and link changes are transactional. Adding an existing set
value or removing a missing one succeeds without changing `updated`. Self-links
are errors. Links are directional.

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
Foreign-key cascades remove tags and incoming and outgoing links.

## Errors

Operational errors write `error: <message>` to stderr, return nonzero, and do
not print to stdout. Stable messages are:

```text
error: run nt init first
error: database is not an nt database
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
error: editor exited unsuccessfully
error: duplicate note id: <id>
error: cannot link note to itself
```

Command grammar errors use clap's stderr diagnostics and exit code. SQLite
internals and host paths are not exposed by expected operational errors.

## Operation

Every mutation uses one short SQLite transaction. Foreign keys are enabled on
all operational connections. WAL lets readers continue from the last committed
snapshot while another connection writes. A contending writer waits for the
bounded busy timeout and then reports the stable retryable error. No transaction
remains open while reading stdin or waiting for `$VISUAL`/`$EDITOR`.
