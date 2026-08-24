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

Append one short immutable memory from stdin or trailing text:

```sh
printf '%s\n' 'Deployment switched to blue-green.' | nt memory add
nt memory add 'Customer prefers weekly summaries.'
```

The success line contains a zero-based sequence number such as `saved #0`.
Memory bodies do not need a Markdown title. They are non-empty single lines of
at most 512 Unicode characters and cannot be edited or removed.

```sh
nt memory recall deployment
nt memory recall 'blue-green'
nt memory wake
```

`recall` joins its arguments with spaces and scans exact raw history for that
case-sensitive literal substring. It returns matching raw entries in sequence
order and does not use FTS, stemming, fuzzy matching, or semantic search.
`wake` prints a deterministic chronological view with at most 128 entries. It
prints all raw memories when they fit; over longer history, old entries are
represented by coarser binary summaries and recent entries are more precise.
If a summary selected for that view is missing, `wake` fails until the caller
creates the needed derived summaries with `nap`.

### Nap And Zoom

Every summary covers an aligned power-of-two range and has two direct children.
The calling agent creates summary text explicitly; `nt` has no work queue,
background worker, or built-in model call. This example assumes raw sequences
0 through 7 already exist:

```sh
nt memory nap
nt memory nap 0-1 'Events zero and one.'
nt memory nap 2-3 'Events two and three.'
nt memory nap 0-3 'Events zero through three.'
nt memory zoom 0-3
```

With no range, `nap` prints the smallest buildable missing range, its two
children, and a command template. With a range and text, it stores the
caller-produced summary. Ranges are half-open internally but inclusive at the
CLI: `0-1` covers raw memories 0 and 1, while `0-7` covers 0 through 7. `zoom`
reveals exactly two direct children without printing the selected summary.
Repeated zooming eventually reaches exact raw entries.

If a summary is wrong, forget it and its dependent ancestors while keeping raw
history and lower descendants:

```sh
nt memory forget 0-1
nt memory nap
```

## Shell Use

```sh
nt find rust | less
nt find rust | head -n 100
nt memory recall deployment | less
```

Redirected note results and memory line output stream one physical line per
record. Closing a list or search pipeline early, as `head` does, is successful.
Note result text fields are JSON encoded, so decode them before reusing them as
command arguments.

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
