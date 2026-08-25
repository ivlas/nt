# Using nt

This guide covers common workflows. See the [CLI reference](cli-reference.md)
for the complete command and output contract.

`nt` keeps editable CommonMark notes in one SQLite database. Every note has one
collection, optional tags, and directional links.

## Start

```sh
cargo install --locked --path .
nt init
```

`nt init` creates the database in the resolved home directory, normally at
`$HOME/.nt/nt.sqlite3`. No other command creates storage. Running `init` again
against a valid database prints `already initialized`.

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
nt read collection:work/nt tag:rust
```

`list` accepts structured filters. `find` adds literal lexical terms. All
expressions are combined with AND, and results are complete unless `limit:` is
present. Common filters include `collection:`, `tag:`, `id:`, `links-to:`,
`linked-from:`, `created-since:`, and `updated-since:`. Timestamps use UTC
seconds, for example `2026-08-22T14:30:12Z`.

Search is deterministic and token-based. SQLite's supported Latin-diacritic
removal makes `cafe` match `café`, but `owner` does not match `ownership`. There
is no fuzzy search, relevance ranking, or hidden semantic search.

`show` writes only one canonical body. `read` streams complete filtered records
as JSONL, including canonical bodies. On redirected stdout, `list` and `find`
write headerless JSON-encoded TSV; see [Note Output](cli-reference.md#note-output)
and [Full Note Output](cli-reference.md#full-note-output).

Resume an external consumer from a completed global revision:

```sh
nt changes since:42
```

Redirected output is JSONL ordered by ascending revision. It reports `add`,
`edit`, `metadata`, and `remove` operations without historical bodies. Process
all rows sharing a revision before checkpointing it; after interruption, rerun
from the last fully processed revision.

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

## Shell Use

```sh
nt find rust | less
nt find rust | head -n 100
nt read tag:rust | jq -r '.body'
nt changes since:0 | jq -c .
```

Redirected results stream one physical line per record. Closing a list, search,
read, or change pipeline early, as `head` does, is successful. Text fields are
JSON encoded, so decode them before reusing them as command arguments.

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
