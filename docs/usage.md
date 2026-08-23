# Using nt

This guide covers common workflows. See the [CLI reference](cli-reference.md)
for the complete command and output contract.

`nt` keeps two kinds of local data in one SQLite database:

- **Notes** are editable CommonMark documents with a collection, optional tags,
  and directional links.
- **Memory** is an immutable sequence of short experiences. Derived summaries
  keep retrieval bounded as history grows.

## Start

```sh
cargo install --locked --path .
nt init
```

`nt init` creates the database in the resolved home directory, normally at
`$HOME/.nt/nt.sqlite3`. No other command creates storage. Running `init` again
against a valid database prints `already initialized`.

## Notes

### Capture

Pipe a multiline note:

```sh
printf '%s\n' '# Rust note' '' 'Ownership details.' | nt add tag:rust
```

Use text after `--` for a short note:

```sh
nt add collection:work/nt tag:rust,sqlite -- '# Storage decision'
```

Run `nt add collection:research/sqlite` interactively, without piped input, to
compose in `$VISUAL` or `$EDITOR`. A note body must start with `# ` followed by
a non-whitespace title. The default collection is `inbox`.

Capture metadata is limited to:

```text
collection:<path>
tag:<tag,...>
link:<id,...>
```

Collections are lowercase paths such as `work/nt`. Tags are lowercase values
such as `rust` or `project_a`. Links are directional.

### Find And Read

Use result rows to select notes before loading exact bodies:

```sh
nt list
nt list collection:work/nt tag:rust
nt list not:tag:archived limit:50
nt list tags
nt list collections
nt find 'ownership borrow' tag:rust
nt show "$NOTE_ID"
```

`list` accepts structured filters. `find` adds literal lexical terms. All
expressions are combined with AND, and results are complete unless `limit:` is
present. Common filters include `collection:`, `tag:`, `id:`, `links-to:`,
`linked-from:`, `created-since:`, and `updated-since:`. Timestamps use UTC
seconds, for example `2026-08-22T14:30:12Z`.

Search is deterministic and token-based. SQLite's supported Latin-diacritic
removal makes `cafe` match `café`, but `owner` does not match `ownership`. There
is no fuzzy search, relevance ranking, or hidden semantic search.

`show` writes only the canonical body. On redirected stdout, `list` and `find`
write headerless JSON-encoded TSV; see [Note Output](cli-reference.md#note-output).

### Edit And Organize

```sh
printf '%s\n' '# Updated title' '' 'Replacement body.' | nt edit "$NOTE_ID"
nt move "$NOTE_ID" work/project_a
nt tag "$NOTE_ID" +storage
nt tag "$NOTE_ID" -storage
nt link "$NOTE_ID" +"$TARGET_ID"
nt link "$NOTE_ID" -"$TARGET_ID"
```

Running `nt edit "$NOTE_ID"` interactively, without piped input, opens the
current body in the configured editor. The update fails if another body edit
commits first. Adding an existing tag or link, or removing a missing one,
succeeds without changing the note. Self-links are rejected.

Remove one or more notes atomically:

```sh
nt rm "$NOTE_ID"
nt rm "$FIRST_ID" "$SECOND_ID"
```

If any ID is invalid, missing, or repeated, no requested note is removed.

## Memory

### Capture And Recall

Append one short immutable memory from stdin or text after `--`:

```sh
printf '%s\n' 'Deployment switched to blue-green.' | nt memory add
nt memory add -- 'Customer prefers weekly summaries.'
```

The success line contains a monotonically increasing sequence number. Memory
bodies do not need a Markdown title and are limited to 1,024 Unicode characters.
They cannot be edited or removed.

```sh
nt memory show "$SEQ"
nt memory show L0:0
nt memory list since:1 limit:100
nt memory recall deployment
nt memory recall deployment since:100
nt memory context deployment
nt memory context
```

`show` returns the exact raw body for a numeric sequence or the exact stored
summary body for a summary node. `list` reads raw history in sequence order.
`recall` searches exact raw history with literal terms and English-oriented
Porter stemming, so forms such as `skill` and `skills` normally match the same
memory. This is not fuzzy or semantic search. `context` uses the same lexical
matching for raw entries and derived summaries, producing deterministic output
of at most 32,768 Unicode characters; it never calls a model.

### Summarize And Expand

Every complete group of 16 children can be summarized. The calling agent does
this explicitly; `nt` has no background worker or built-in summarizer. This
example assumes raw sequences 1 through 16 already exist:

```sh
nt memory pending
nt memory pending L0:0
printf '%s\n' 'Factual summary of the supplied children.' |
  nt memory summarize L0:0
nt memory show L0:0
nt memory expand L0:0
```

`pending L0:0` prints the 16 children and a compression instruction. `summarize`
stores the caller-produced result. `show` inspects that stored summary, while
`expand` reveals exactly one child level beneath it without including the
selected summary. Repeating expansion through lower nodes eventually reaches
exact raw entries.

If a summary is wrong, invalidate it and its dependent ancestors while keeping
raw history:

```sh
nt memory invalidate L0:0
nt memory status
```

## Shell Use

```sh
nt find rust | less
nt find rust | head -n 100
nt memory recall deployment | less
```

Redirected note results and raw memory results stream one physical line per
record. Closing a list or search pipeline early, as `head` does, is successful.
Text fields are JSON encoded, so decode them before reusing them as command
arguments.

## Backups

Use SQLite's backup mechanism while any `nt` or SQLite process may be active.
This example uses the normal `HOME` location; substitute the resolved database
path when `nt` uses `USERPROFILE`. Use a new destination path unless replacing
an existing backup is intentional:

```sh
sqlite3 "$HOME/.nt/nt.sqlite3" ".backup '/path/to/nt-backup.sqlite3'"
```

Do not copy only `nt.sqlite3` while connections are active; committed changes
may still be in `nt.sqlite3-wal`. For a filesystem copy, first close every
process using the database and checkpoint the WAL.
