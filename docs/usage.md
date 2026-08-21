# Using nt

`nt` stores canonical CommonMark notes and metadata in
`$HOME/.nt/nt.sqlite3`. The database is local and directly inspectable with
SQLite tools. Direct database writes are unsupported; use `nt` commands for
mutations. Markdown remains ordinary CommonMark without nt-specific body syntax.

## Install And Initialize

```sh
cargo install --path .
nt init
```

Only `nt init` creates storage. Running it again against a valid database prints
`already initialized`. Other commands return `run nt init first` rather than
creating files.

## Capture

Pipe exact multiline content:

```sh
printf '%s\n' '# Rust note' '' 'Ownership details.' | nt add tag:rust
```

Use text after `--` for short bodies:

```sh
nt add collection:work/nt tag:rust,sqlite -- '# Storage decision'
```

Run `nt add collection:research/sqlite` with terminal stdin to compose in
`$VISUAL` or `$EDITOR`. Capture uses exactly one of trailing text, piped stdin,
or the editor. The first line must be a non-empty `# Title`. Collection defaults
to `inbox`.

Capture metadata is limited to:

```text
collection:<path>
tag:<tag,...>
link:<id,...>
```

Collections are lowercase paths such as `inbox`, `work/nt`, or
`research/sqlite`. A note belongs to exactly one collection. Tags are optional,
and links are directional.

## Find And Read

Use summaries before loading exact bodies:

```sh
nt list
nt list collection:work/nt tag:rust
nt list collection:work/nt limit:50
nt list not:tag:archived
nt list linked-from:<source-id>
nt list tags
nt list collections
nt find 'ownership borrow' tag:rust
nt find rust limit:100
nt show <id>
```

`list` accepts structured filters. `find` accepts the same filters and literal
lexical terms. All expressions are AND-combined. Filters are:

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

`list` and `find` return every match unless an explicit `limit:` is supplied.
The limit selects the first N summaries in the normal deterministic order.
`links-to:<target>` finds notes that point to a target; `linked-from:<source>`
finds notes pointed to by a source. These filters describe the returned notes,
inspect direct edges only, and require full canonical note IDs. A missing source
or a source with no outgoing links returns no matches. Summaries keep the
existing `outgoing` count column; they do not add an incoming-link count.

Lexical search uses complete Unicode tokens. Punctuation separates terms and
has no FTS operator meaning. Terms may occur in any order. Supported Latin
diacritics are removed, so `cafe` matches `café`. Prefixes are not expanded, so
`owner` does not match `ownership`.

On a terminal, list and find summaries have an aligned header row. Redirected
list and find rows contain JSON-encoded `id`, `updated`, `collection`, `title`,
and `tags` cells followed by a numeric `outgoing` link count, separated by tabs.
Every note occupies one physical output line. Metadata inventories contain one
current, distinct value per line in lexical order; redirected values are JSON
strings. `show` writes only the exact canonical body.

## Edit And Organize

Replace the complete body from stdin:

```sh
printf '%s\n' '# Updated title' '' 'Replacement body.' | nt edit <id>
```

Run `nt edit <id>` with terminal stdin to edit the current body in `$VISUAL` or
`$EDITOR`. The update is rejected if another body edit commits first. Tag,
collection, and link changes do not conflict with an open editor.

```sh
nt move <id> work/project_a
nt tag <id> +storage
nt tag <id> -storage
nt link <id> +<target-id>
nt link <id> -<target-id>
```

A real `nt link` addition or removal updates its source note, not its target.
Adding an existing tag or link and removing a missing one succeeds without
changing update timestamps. Self-links are rejected; links remain directional.

## Library Evidence

Library stores external evidence while Notes store authored synthesis:

```sh
curl -sS https://sqlite.org/wal.html | nt library add https://sqlite.org/wal.html 'SQLite WAL'
nt library find 'write ahead'
nt library show <library-id>
curl -sS https://sqlite.org/wal.html | nt library capture <library-id>
printf '%s' 'Manual summary' | nt library summary <library-id>
nt library history <library-id>
nt ref <note-id> <library-id>
nt unref <note-id> <library-id>
```

Captures preserve exact supplied content and are immutable. Re-capturing
identical content is idempotent; changed content appends history. Default search
uses only each item's latest capture, so stale historical text does not appear as
current evidence. Summaries belong to exact captures and are never carried onto
new content.

## Remove

```sh
nt rm <id>
nt rm <id> <id> <id>
```

Removal validates every ID before deleting any note. Missing or duplicate IDs
leave all requested notes intact. Before cascades remove incoming links,
surviving source notes receive a new update timestamp because their outgoing-link
sets changed. Deleting a source does not update its outgoing targets.

## Resources And Backups

`nt` reads each captured or edited body completely into memory before opening a
write transaction. It does not impose an application-level body-size limit;
available memory and disk space are the operational limits. SQLite also uses
disk space for its lexical index and temporary WAL state, so check available
resources before importing unusually large input.

Use SQLite's backup mechanism for a consistent backup while `nt` or another
SQLite connection may be active:

```sh
sqlite3 "$HOME/.nt/nt.sqlite3" ".backup /path/to/nt-backup.sqlite3"
```

Do not copy only `nt.sqlite3` while connections are active because committed
changes may still be in `nt.sqlite3-wal`. For an offline filesystem copy, close
all `nt` and SQLite processes, run `PRAGMA wal_checkpoint(TRUNCATE)`, close that
SQLite connection, and then copy the main database file.

## Shell Composition

```sh
nt find rust | less
nt find rust | head -100
nt find rust | fzf --preview 'nt show {1}'
nt list tag:decision | cut -f1
```

Redirected note summaries stream as they are read. If a consumer such as `head`
closes the pipe early, `nt` stops retrieval without reporting an error.

Agents use the same CLI. Prefer `list` or `find` for candidate construction,
then `show` only selected IDs to bound context. Obtain user approval before
mutations.
