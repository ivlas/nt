# nt CLI Reference

`nt` is a flagless, configless local CLI over `$HOME/.nt/nt.sqlite3`. SQLite is
canonical for editable note bodies and metadata and for immutable raw memory.
Notes and memory are separate first-class concrete models.

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
nt memory add [-- body...]
nt memory show <seq>
nt memory list [since:<seq>] [until:<seq>] [limit:<n>]
nt memory recall <term-or-filter...>
nt memory context [term...]
nt memory pending [L<level>:<block>|limit:<n>]
nt memory summarize <node> [-- summary...]
nt memory expand <node>
nt memory invalidate <node>
nt memory status
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

Schema version 2 uses the alpha clean-sheet policy. Version 1 is not migrated in
place; an incompatible development database must be deleted and initialized
again. The flat compile-time manifest contains application, note, and memory
schema contributions.

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

## Persistent Memory

Persistent memory is a separate first-class model from notes. Its fixed
architecture is:

```text
immutable raw experience -> 16-way summary pyramid -> indexed retrieval
-> 32 KiB context compiler -> progressive expansion -> exact original history
```

Raw memory bodies are canonical and immutable. Summaries, summary jobs, raw FTS,
and summary FTS are derived and rebuildable. The calling agent performs
summarization explicitly with `pending` and `summarize`; append does not wait for
it. Retrieval makes no model call and requires no embeddings. There is no daemon
or background worker.

Memory v1 has fixed compile-time limits and no configuration:

| Limit | Value |
| --- | ---: |
| Raw body | 1,024 Unicode characters |
| Summary | 1,024 Unicode characters |
| Context memory content | at most 32,768 Unicode characters |
| Summary fanout | 16 |

Storage size and context size are independent. The raw sequence can continue to
grow while each context invocation remains bounded.

### Add And Show

```text
nt memory add [-- body...]
nt memory show <seq>
```

`memory add` accepts a body from exactly one source: non-empty arguments after
`--` or stdin. It never opens `$VISUAL` or `$EDITOR`. Trailing arguments are
joined with one ASCII space. If trailing text is present, non-empty redirected
stdin conflicts; an empty redirect does not. Without trailing text, stdin is
read to EOF and empty input is an error, including when stdin is a terminal.

The body is non-empty UTF-8 text with CRLF and CR normalized to LF. It may not
contain NUL and may contain at most 1,024 Unicode characters. No CommonMark H1
or other note-body rule applies. SQLite assigns the next positive, monotonically
increasing sequence number. Success prints:

```text
saved <seq>
```

`memory show` requires a positive decimal sequence number and writes the exact
canonical raw body with no metadata wrapper or added newline. A missing row is
`error: memory not found: <seq>`.

### List And Recall

```text
nt memory list [since:<seq>] [until:<seq>] [limit:<n>]
nt memory recall <term-or-filter...>
```

The available memory filters are:

```text
since:<positive-seq>
until:<positive-seq>
limit:<positive-integer>
```

`since:` and `until:` are inclusive. If both are present, `since:` may not
exceed `until:`. Each filter may occur at most once. `memory list` accepts only
these filters, orders by `seq ASC`, and returns all matching raw rows unless a
SQL-backed limit is supplied.

`memory recall` requires at least one lexical token in addition to any filters.
Terms are split into Unicode letter-or-digit runs, sorted, deduplicated, quoted
as FTS literals, and AND-combined. Unknown filter-looking expressions are
errors. Matching is complete-token and uses SQLite `unicode61` with Latin
diacritic removal where supported. Results are ordered by
`bm25(memory_fts) ASC, seq ASC`: lower BM25 is more relevant and ascending
sequence breaks relevance ties deterministically. The explicit positive limit,
if any, is applied by SQLite.

Both commands output one raw memory per physical line with these TSV columns:

```text
seq    created    body
```

`seq` is an unquoted JSON number. `created` and `body` are JSON-encoded strings,
so tabs, newlines, quotes, and backslashes in text do not create extra columns
or physical lines:

```text
<seq>\t"<created>"\t"<body>"
```

This format is headerless and unchanged on a TTY. Output is streamed, and a
downstream pipe closing early is successful termination. There is no raw FTS
syntax, prefix expansion, fuzzy matching, semantic search, model call, or
embedding lookup.

### Summary Work

```text
nt memory pending [L<level>:<block>|limit:<n>]
nt memory summarize <node> [-- summary...]
```

A summary node uses canonical `L<level>:<block>` syntax with non-negative
decimal integers and no leading zeroes except `0`. Level zero covers 16 exact raw
entries. Each higher node covers 16 summaries from the preceding level.

A level-zero job becomes pending when its complete group of 16 raw entries has
been appended. A higher-level job becomes pending when all 16 child summaries
exist. Children may be summarized out of order; completion of the final child
creates or repairs the parent job. Pending work is explicit queue state and no
worker consumes it automatically.

With no argument, `memory pending` lists every job ordered by `level ASC,
block ASC`. `limit:<n>` applies a positive bound. Each output row is plain,
headerless TSV:

```text
node\traw-start-raw-end\tlevel
```

For example:

```text
L0:0\t1-16\t0
```

`memory pending <node>` requires that exact node to be pending and prints a
self-contained compression task. Its header is:

```text
node\t<node>
raw range\t<start>-<end>
level\t<level>
```

After a blank line it prints exactly 16 children using the expansion formats
below, then a fixed factual-compression instruction ending in `Maximum: 1024
characters.` Inspecting pending work is read-only and does not reserve, claim,
or remove the job.

`memory summarize` reads the summary only from trailing arguments after `--` or
stdin, using the same no-editor, source-conflict, newline normalization, NUL,
and Unicode-character rules as `memory add`. A new summary requires a pending
node and all 16 expected children. One transaction inserts the derived summary,
removes the job, and makes the parent pending when all parent children exist.
Success prints:

```text
summarized <node>
```

Submitting the same text again for an already completed node is idempotent and
prints the same success line. Submitting different text for that node is a
conflict and does not replace the existing summary. A node with neither a
pending job nor an existing identical summary is rejected.

### Context

```text
nt memory context [term...]
```

Terms use the same literal tokenization as recall. Filter-looking expressions
such as `since:1` are errors. Zero lexical tokens select queryless context.
Retrieval is deterministic and has no model call.

Every candidate query has a fixed SQL `LIMIT 256`. With no lexical terms, the
32,768-character memory-content budget is split 60% to recent exact raw entries
ordered `seq DESC` and 40% to broad summaries ordered `level DESC, block DESC`.

With lexical terms, the budget is split 40% to lexical raw entries, 30% to
recent raw entries, and 30% to lexical summaries. Lexical raw order is BM25
ascending then `seq DESC`; lexical summary order is BM25 ascending then
`level DESC, block DESC`. Integer remainder goes to the final pool.

Candidates that do not fit both their pool and the total budget are skipped;
items are never truncated and unused pool capacity is not transferred. Exact
raw sequences are deduplicated across pools. A summary is rejected if its raw
range overlaps a selected raw entry or summary. Exact raw is selected before
summaries and therefore wins overlap. Final output is ordered chronologically by
raw range.

The content budget includes every complete selected raw body and summary. It
excludes generated document labels, timestamps, ranges, separators, and
formatting newlines. Raw items render as:

```text
# memory <seq> (<created>)
<complete raw body>
```

Summary items render as:

```text
# summary <node> (<raw-start>-<raw-end>)
<complete summary>
```

The renderer writes a newline after each complete body and one additional
newline before each item after the first. A stored trailing newline is preserved,
so it can produce additional visible blank space. The fixed candidate bounds
make context predictable and SQL-bounded but not exhaustive over all history.
Literal search does not recover synonyms, and a summary can omit or misstate
facts supplied by its calling agent. Use expansion and raw history when exact
evidence matters.

### Expand And Invalidate

```text
nt memory expand <node>
nt memory invalidate <node>
```

`memory expand` requires an existing summary and reveals exactly one level of
its 16 children in ascending sequence or block order. Level-zero children use
the raw list format:

```text
seq\t"created"\t"body"
```

Higher-level children use four headerless TSV columns:

```text
node\traw-start-raw-end\t"created"\t"summary"
```

The timestamp and textual content are JSON strings; sequence, node, and range
are not JSON-quoted. Repeating expansion through lower summary nodes reaches
the exact immutable raw entries.

`memory invalidate` removes the selected derived summary and every stored
ancestor that depends on it, removes their stale jobs, and requeues the selected
node if its 16 children are complete. Raw memories are always preserved. The
node must name an existing summary. Success prints:

```text
invalidated <node>
```

### Status

```text
nt memory status
```

Status is a fixed, unquoted two-column TSV report:

```text
raw memory count\t<count>
highest sequence\t<seq|none>
summary count\t<count>
pending summary count\t<count>
highest completed level\t<level|none>
raw FTS ready\t<true|false>
summary FTS ready\t<true|false>
```

The FTS fields report whether the corresponding virtual tables exist. They do
not perform a full content-integrity audit or rebuild derived state.

## Errors

Operational errors write `error: <message>` to stderr. Exit codes are `2` for
invalid syntax, queries, or values; `3` for missing storage, notes, or memories;
`4` for retryable database contention or concurrent edits; and `1` for other
operational failures. Success and an intentionally closed streamed list, find,
or memory list/recall output pipe return `0`. Errors before success-output
writing do not print to stdout. Stable messages are:

```text
error: run nt init first
error: home directory not found
error: database is not an nt database
error: database is corrupt
error: database could not enter WAL mode
error: system clock is outside the supported timestamp range
error: stored note is invalid (<safe note or row identity>, field: <field>)
error: stored memory is invalid (<safe sequence or segment identity>, field: <field>)
error: unsupported nt schema version <version>; delete ~/.nt/nt.sqlite3 and run nt init
error: database is busy; retry
error: note not found: <id>
error: memory not found: <seq>
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
create a duplicate note. Blindly retrying `memory add` can append a duplicate
raw memory.

During SQLite inspection, `database is not an nt database` is reserved for
recognized application-identity or schema-shape mismatches. Corrupt or malformed
database images use the stable corruption message, and busy or locked databases
use the retryable busy message. Other unexpected SQLite failures retain
diagnostic detail rather than being misreported as foreign databases. Command
grammar errors use clap's stderr diagnostics and exit code `2`.

## Operation

Every mutation uses one short SQLite transaction. Foreign keys are enabled on
all operational connections.

The read-only note commands are `show`, `list`, and `find`. The read-only memory
commands are `memory show`, `memory list`, `memory recall`, `memory context`,
`memory pending`, `memory expand`, and `memory status`. They open SQLite in
read-only mode and work when the database file is not writable, provided SQLite
can read the database and any WAL state it references.

The write note commands are `add`, `rm`, `edit`, `move`, `tag`, and `link`. The
write memory commands are `memory add`, `memory summarize`, and `memory
invalidate`. `nt init` and write commands require a writable database and
establish WAL. WAL lets readers continue from the last committed snapshot while
another connection writes. A contending writer waits for the bounded busy
timeout and then reports the stable retryable error. No transaction remains open
while reading stdin or waiting for `$VISUAL`/`$EDITOR`.

Running `nt` without a command and `nt help [command...]` do not open storage.
