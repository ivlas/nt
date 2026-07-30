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
namespace with `nt init work`. `nt config vault` lists all vaults; there is no
active vault to switch.

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

Use cheap projections before loading full bodies into an agent context:

```sh
nt list id,title,home
nt list id,title,status status:open
nt list tags
nt list collections
nt find collection:personal/rust tag:rust
nt find body:'ownership borrow'
nt show <id>
```

This supports bounded context construction: list and find return compact,
deterministic one-record-per-line candidates, while show retrieves one exact
body. Quoted multiword `body:` values match all terms, not an exact phrase.
Body search reads the canonical text stored in SQLite.

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
nt agenda week
```

## Edit, Remove, And Export

```sh
nt open <id>
nt rm <id>
nt export archive <id>
```

`open` uses a temporary Markdown file only as the `$EDITOR` interface; the
committed body lives in SQLite. `rm` removes dependent memberships and links in
one transaction. `export` creates portable Markdown snapshots.

## User-Directed Agent Use

Agents use the same CLI. Prefer `list` or `find` for small candidate records,
then `show` only selected ids to limit token use. Before any mutation, present
the draft or exact update commands and obtain user approval. The supported
workflow is one user-directed writer at a time.
