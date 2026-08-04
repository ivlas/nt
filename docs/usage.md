# Using nt

`nt` stores all canonical knowledge in `$HOME/.nt/nt.sqlite3`. The database is
local, portable, and directly inspectable with SQLite tools. Markdown remains
the body format and export format, but there is no filesystem vault or JSON
index.

## Install And Initialize

```sh
cargo install --path .
nt init personal
```

This creates logical vault `personal` and `personal/inbox`. Add another logical
namespace with `nt init work`. There is no configuration or active vault to
switch.

## Capture

With one vault, fast capture defaults to its inbox:

```sh
printf '%s\n' '# Rust note' '' 'Ownership details.' | nt note tag:rust
```

Use a qualified home when organizing or after creating multiple vaults:

```sh
cat <<'EOF' | nt note home:personal/rust collection:work/project_a
# Shared implementation note

The body remains ordinary CommonMark.
EOF
```

The first `collection:` is home when `home:` is omitted. Other collections are
references, including collections owned by other vaults.

## Find And Read

Use compact projections before loading full bodies into an agent context:

```sh
nt list id,title,home
nt list id,title,status status:open
nt list tag
nt list collection
nt find collection:personal/rust tag:rust
nt find body:'ownership borrow'
nt show <id>
```

This supports bounded context construction: `find` returns one compact summary
per line, redirected `list` output is headerless and tab-separated, and `show`
retrieves one exact body. Interactive `list` output includes aligned headers.
Set-valued projections such as `tag` and `collection` remain one row per note
and render their values comma-separated.
Bare, `title:`, and `body:` searches use whole Unicode tokens from the FTS5
index. Punctuation separates terms, all terms are required in any order,
Unicode case is folded, and `unicode61` removes supported Latin diacritics
(`cafe` matches `café`). Shell quoting groups a multiword value but does not make
it a phrase. Prefixes are not expanded, so `body:owner` does not match
`ownership`. `source:` remains a case-insensitive SQL substring search.

Candidate filtering and compact projection happen in SQLite. `find` retrieves
only id, creation time, title, and tags for matching notes. Lexical predicates
consult the derived index without loading note bodies into Rust.

Normal shell composition remains available:

```sh
nt find rust | less
nt find rust | fzf --preview 'nt show {1}'
nt list id | fzf --multi | xargs -n1 nt show
```

## Organize

```sh
nt update <id> home work/project_a
nt update <id> collection +personal/rust
nt update <id> collection -personal/inbox
nt update <id> tag +storage
```

Home is canonical and always remains a membership. Moving home preserves the old
home as a reference until explicitly removed.

## Todos

```sh
printf '%s\n' '# Release' | nt todo home:work/project_a priority:A
nt update <id> due 2026-08-15
nt agenda
nt agenda week
nt list id,title,priority,tag,home kind:todo status:open
nt list id,title,tag,home kind:todo status:waiting
```

`agenda` is date-focused: the default shows overdue and today, while `week` adds
the next six days. Use ordinary `list` projections for open todos, waiting
items, inbox collections, and `tag:someday` items.

## Remove And Export

```sh
nt rm <id>
nt export archive <id>
```

`rm` validates all requested ids before deleting and removes dependent
memberships, tags, sources, and links in one transaction. `export` creates
portable Markdown snapshots; each file is replaced atomically, but a multi-file
export is not one transaction.

## User-Directed Agent Use

Agents use the same CLI. Prefer `list` or `find` for small candidate records,
then `show` only selected ids to limit token use. Before any mutation, present
the draft or exact update commands and obtain user approval. The supported
workflow is one user-directed writer at a time.
