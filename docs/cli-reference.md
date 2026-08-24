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
- Memory IDs are zero-based decimal sequence numbers.
- Memory ranges use canonical non-negative decimals, such as `0-1` and `0-7`.
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
nt memory add [memory...]
nt memory wake
nt memory recall <pattern...>
nt memory nap
nt memory nap <range> [summary...]
nt memory zoom <range>
nt memory forget <range>
nt help [command...]
```

Running `nt` is equivalent to `nt help`. The CLI is flagless: `-h`, `--help`,
`-V`, and `--version` are not aliases and are rejected.

Reference sections: [storage](#storage), [note input](#note-input),
[note queries](#note-queries), [note mutations](#note-mutations),
[memory input](#memory-input), [wake and recall](#wake-and-recall),
[binary summaries](#binary-summaries), [zoom and forget](#zoom-and-forget), and
[errors](#errors-and-operation).

## Storage

`nt init` is the only command that creates the storage directory, database, or
schema. It prints `initialized` for a new database and `already initialized`
for a valid existing database. The resolved home path must be non-empty and
absolute.

Ordinary commands never create or repair storage. They require application ID
`0x4e544e54`, schema version `4`, and the expected schema definitions. There is
no migration system; incompatible databases are rejected.

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

`memory add` reads one non-empty body from trailing arguments or stdin and never
opens an editor. Arguments are joined with one ASCII space. One final LF or CRLF
from stdin is removed; any remaining CR or LF and every NUL are rejected. The
body may contain at most 512 Unicode characters, and note title rules do not
apply. Conflicting non-empty stdin and arguments are an error.

SQLite stores raw memories as one immutable, contiguous sequence beginning at
zero. Success prints `saved #<sequence>`.

## Wake And Recall

```text
nt memory wake
nt memory recall <pattern...>
```

`recall` joins its arguments with one ASCII space and performs a case-sensitive
literal substring scan over canonical raw history. It does not tokenize, stem,
rank, or search summaries. Matching raw memories are emitted chronologically.

`wake` emits a deterministic chronological cover of all history. Its only bound
is the compile-time `WAKE_ENTRIES = 128` entry count. If history has at most 128
entries, every item is raw. For larger history, the aligned dyadic range with
the greatest size relative to its age is refined until the budget is filled.
This produces progressive age decay: old history is coarser and recent history
is more precise without concentrating the budget in a long raw tail. Every
selected derived summary must exist; otherwise `wake` fails and directs the
caller to `memory nap`.

Both commands stream one item per physical line:

```text
#<sequence> <raw-body>
#<inclusive-range> <summary-body>
```

`recall` emits only the first form. Bodies are already constrained to one line,
so output needs no escaping. A downstream pipe closing early is successful.

## Binary Summaries

Summary rows use half-open ranges `[lo, hi)` internally. A valid range has a
power-of-two size of at least two and `lo` is aligned to that size. The CLI
renders the same range with an inclusive upper bound:

```text
internal [0, 2)   -> CLI 0-1
internal [0, 8)   -> CLI 0-7
internal [4, 8)   -> CLI 4-7
```

Each summary has exactly two direct children. A size-two range has two raw
children; every larger range has two summary children of half its size.

`memory nap` derives the smallest buildable missing summary, breaking equal-size
ties by the lowest start. A size-two range is buildable when both raw memories
exist. A larger range is buildable when both direct child summaries exist. With
no work it prints `nothing to nap`.

For work, `nap` prints the selected inclusive range, its two complete children,
and a command template. It does not persist a claim or create a work record.
The caller supplies the result with `memory nap <range> [summary...]`; input and
validation match raw memory input, including the 512-character single-line
limit. The success line identifies the stored range. Repeating identical text
is idempotent; different text for an existing range is a conflict.

Adding raw memory never waits for derived summaries. Memory has no FTS, work
tables, background processing, built-in model calls, or semantic retrieval.

## Zoom And Forget

`memory zoom <range>` requires an existing summary and emits exactly its two
direct children in chronological order. It does not emit the selected summary.
For a size-two summary the children are raw; for a larger summary they are the
two stored child summaries.

`memory forget <range>` requires an existing summary and transactionally deletes
that summary plus every stored ancestor that contains it. It never deletes raw
memories, descendants, or unrelated summaries. Success prints `forgot
#<range>`.

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
