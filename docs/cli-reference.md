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
- Memory IDs are positive decimal sequence numbers assigned by SQLite.
- Summary nodes use `L<level>:<block>`, with canonical non-negative decimals.
- Timestamps are UTC seconds such as `2026-08-22T14:30:12Z`.
- Character limits count Unicode characters, not UTF-8 bytes.
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
nt find <term-or-filter...>
nt rm <id...>
nt edit <id> [-- body...]
nt move <id> <collection>
nt tag <id> <+tag|-tag>
nt link <id> <+id|-id>
nt memory add [-- body...]
nt memory show <seq|node>
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

Running `nt` is equivalent to `nt help`. The CLI is flagless: `-h`, `--help`,
`-V`, and `--version` are not aliases and are rejected.

Reference sections: [storage](#storage), [note input](#note-input),
[note queries](#note-queries), [note mutations](#note-mutations),
[memory input](#memory-input), [memory queries](#memory-queries),
[summary work](#summary-work), [memory context](#memory-context),
[expansion and status](#expand-invalidate-and-status), and
[errors](#errors-and-operation).

## Storage

`nt init` is the only command that creates the storage directory, database, or
schema. It prints `initialized` for a new database and `already initialized`
for a valid existing database. The resolved home path must be non-empty and
absolute.

Ordinary commands never create or repair storage. They require application ID
`0x4e544e54`, schema version `3`, and the expected schema definitions. This
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
An editor update is rejected if another body edit commits first. Success prints
`updated <id>`.

## Note Queries

`show <id>` writes the exact canonical body with no wrapper or added newline.

`list` accepts these structured expressions; `find` accepts the same expressions
plus one or more lexical terms:

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

## Note Mutations

```text
nt move <id> <collection>    -> moved <id> <collection>
nt tag <id> <+tag|-tag>      -> tagged <id> <operation>
nt link <id> <+id|-id>       -> linked <id> <operation>
nt rm <id...>                -> removed <count>
```

These changes are transactional. Adding an existing set value or removing a
missing one succeeds without changing `updated`. Links are directional;
self-links are errors, and a real link change updates only its source note. Link
addition requires an existing target. Removing a valid target ID that is not
stored succeeds as a no-op.

`rm` rejects duplicate IDs and validates every ID before deleting any note.
Deleting a target updates surviving source notes whose outgoing links are
removed. Deleting a source does not update its targets.

## Memory Input

`memory add` reads one non-empty body from arguments after `--` or stdin. It
never opens an editor. Newlines are normalized to LF; NUL is rejected; the body
may contain at most 1,024 Unicode characters. Note title rules do not apply.
Success prints `saved <seq>`.

`memory show <seq|node>` writes exact stored content with no wrapper or added
newline. A numeric sequence writes the raw-memory body; a summary node writes
the stored summary body. Raw memories are immutable.

## Memory Queries

```text
nt memory list [since:<seq>] [until:<seq>] [limit:<n>]
nt memory recall <term-or-filter...>
```

`since:` and `until:` are inclusive. Each filter may occur once, and `since:`
cannot exceed `until:`. `list` orders raw rows by `seq ASC`.

`recall` requires at least one lexical token and accepts the same filters.
Memory FTS uses SQLite FTS5 Porter stemming over Unicode61, so common English
forms such as `skill` and `skills` can match the same raw memory. Porter is
primarily English-oriented and does not promise morphological normalization for
arbitrary languages. Queries remain literals: there is no raw FTS syntax,
prefix expansion, ranking, fuzzy matching, or semantic search. Results are
ordered by `seq ASC`; limits are positive and applied by SQLite.

Both commands stream headerless TSV with one raw memory per physical line:

```text
<seq>\t"<created>"\t"<body>"
```

The sequence is an unquoted number; timestamp and body are JSON strings. This
format is unchanged on a TTY. A downstream pipe closing early is successful.

## Summary Work

Memory summaries form a fixed 16-way tree. `L0:0` covers raw sequences `1-16`;
each higher node covers 16 summaries from the level below.

`memory pending` lists eligible jobs in `level ASC, block ASC` order. An
optional `limit:<n>` must be positive:

```text
<node>\t<raw-start>-<raw-end>\t<level>
```

`memory pending <node>` requires that exact pending job and prints its node,
range, level, 16 complete children, and a factual-compression instruction:

```text
node\t<node>
raw range\t<raw-start>-<raw-end>
level\t<level>

<16 children in expansion format>

Compress these children into one factual summary.
Keep durable information.
Preserve decisions, outcomes, constraints and important changes.
Invent nothing.
Maximum: 1024 characters.
```

It is read-only and does not claim the job.

`memory summarize <node>` reads a caller-produced summary from arguments after
`--` or stdin. The same newline, NUL, and 1,024-character rules as raw memory
apply. A new summary requires a pending node and all 16 children. Success prints
`summarized <node>`. Repeating the same summary is idempotent; different text for
an existing node is a conflict.

Summarization is explicit. Appending does not wait for it, and `nt` has no
background worker or built-in model call.

## Memory Context

`memory context [term...]` compiles deterministic context from complete raw
entries and summaries. Terms use the memory lexical matching above; filter
expressions are not accepted. Terms influence selection rather than filtering
every output item: context also includes recent raw history and may fill unused
space with broad summaries. With no terms, it combines recent raw history with
broad summary coverage.

The complete stdout document is limited to 32,768 Unicode characters, including
headers, timestamps, ranges, separators, and newlines. Items that do not fit are
skipped, never truncated. Selected ranges do not overlap, exact raw evidence
wins conflicts, and final output is chronological. Retrieval uses fixed,
SQL-bounded candidate sets and is therefore not exhaustive over all history.

Raw items render as:

```text
# memory <seq> (<created>)
<complete raw body>
```

Summary items render as:

```text
# summary <node> (<raw-start>-<raw-end>)
<complete summary>
```

Literal matching can miss synonyms, and caller-produced summaries can omit or
misstate facts. Use raw recall and expansion when exact evidence matters.

## Expand, Invalidate, And Status

`memory expand <node>` requires an existing summary and reveals exactly one
level of 16 children beneath it in ascending sequence or block order. It does
not include the selected summary itself. Level-zero children use the raw-memory
TSV format. Higher children use:

```text
<node>\t<raw-start>-<raw-end>\t"<created>"\t"<summary>"
```

`show` inspects the item at the supplied address; `expand` inspects a summary's
direct children.

`memory invalidate <node>` removes the selected summary, every stored ancestor
that depends on it, and stale jobs. It requeues the selected node when its
children are complete and never removes raw memories. Success prints
`invalidated <node>`.

`memory status` writes fixed, unquoted two-column TSV:

```text
raw memory count\t<count>
highest sequence\t<seq|none>
summary count\t<count>
pending summary count\t<count>
highest completed level\t<level|none>
```

The count is the number of raw rows; highest sequence is the greatest assigned
identity and can differ for a manually damaged or inspected database. Schema
opening validates required FTS objects before status runs; status is not an
integrity audit or repair command.

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
| `3` | Missing storage, note, or memory |
| `4` | Retryable database contention or edit conflict |

Common stable messages include `run nt init first`, `database is not an nt
database`, `database is corrupt`, `database is busy; retry`, `note not found:
<id>`, `memory not found: <seq>`, and `note changed while editing: <id>`. Clap
owns command-grammar diagnostics.

Database commits and success-output writes cannot be atomic. If output fails
after commit, `nt` reports that the operation committed but acknowledgment
failed. Retrying `add` or `memory add` blindly can create a duplicate.

Read commands require readable storage; mutations require writable storage.
Connection and transaction mechanics are described in
[Architecture](architecture.md#command-lifecycle).
