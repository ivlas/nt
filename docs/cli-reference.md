# nt CLI Reference

`nt` is a flagless, local CLI over `$HOME/.nt/nt.sqlite3`. SQLite is canonical
for note bodies, metadata, vaults, collections, and relationships.

## Commands

```text
nt init <vault>
nt note [metadata...]
nt todo [metadata...]
nt list [projection] [filter...]
nt find <expr...>
nt show <id>
nt open <id>
nt rm <id...>
nt update <id> <field> <value>
nt agenda [today|week|overdue|waiting|undated]
nt export <path> [id...]
nt config show
nt config vault
nt help [command...]
```

Running `nt` is equivalent to `nt help`. Use `nt help`, not help flags.

## Ids

Notes, vaults, and collections use canonical lowercase UUIDv7 identifiers.
Commands print full note ids; `id:<prefix>` accepts a hexadecimal UUID prefix.

## Vaults And Collections

`nt init personal` creates logical vault `personal` and collection
`personal/inbox`. Vaults are database namespaces, not directories. There is no
active-vault state and reads operate across the database.

A collection belongs to exactly one vault and uses the qualified form
`<vault>/<collection>`, such as `personal/rust` or `work/project_a`. Collection
names may contain additional `/` separators after the vault.

A note has exactly one home collection and may have additional memberships in
any vault. The home is always also a membership. For capture:

- `home:<vault>/<collection>` sets home explicitly.
- The first `collection:` value becomes home when `home:` is absent.
- With exactly one vault and no collection metadata, `<vault>/inbox` is home.
- With multiple vaults, capture requires an explicit home or collection.

Unknown collections are created when their vault exists. Unknown vaults are
errors. Move home with `nt update <id> home <vault>/<collection>`. The old home
remains a reference until removed with a collection update.

## Capture

`note` and `todo` read CommonMark from stdin or `$EDITOR`. The first non-empty
line must be `# Title`. Success prints `saved <id>`.

```sh
printf '%s\n' '# Ownership' '' 'Borrow checker notes.' \
  | nt note home:personal/rust tag:rust

printf '%s\n' '# Release' '' 'Ship.' \
  | nt todo home:work/project_a priority:A due:2026-08-15
```

Note metadata is `home:`, `tag:`, `collection:`, `link:`, and `source:`. Todo
also accepts `status:`, `priority:`, `scheduled:`, and `due:`. New todos default
to `status:open`. Set metadata is repeatable and comma-separated except source,
where commas are literal.

## List And Find

```text
nt list
nt list all [filter...]
nt list <field>[,<field>...] [filter...]
nt list ids
nt list titles
nt list tags [tag]
nt list collections [collection]
nt list sources [source]
nt list links [filter...]
```

Fields are `id`, `home`, `created`, `updated`, `title`, `kind`, `status`,
`priority`, `scheduled`, `due`, `closed`, `tag`, `collection`, `link`, and
`source`. Redirected projections are headerless tab-separated rows.

`find` expressions are case-insensitive and AND-combined:

```text
<word> #<tag> id:<prefix> tag:<tag> title:<term>
day:<date> since:<date> before:<date> kind:<kind> status:<status>
priority:<priority> scheduled:<date> due:<date> closed:<date>
collection:<vault>/<collection> link:<id> source:<term> body:<term> not:<expr>
```

Body search reads body text from SQLite. Quoted multiword `body:` values match
all terms, not an exact phrase. Retrieval is deterministic and unranked; there
is no scoring, fuzzy search, embeddings, or semantic search.

## Show And Open

`show` prints id, title, home, timestamps, metadata, then the exact body. `open`
copies the body to a temporary file for `$EDITOR`, validates it, detects a
concurrent update by timestamp, and commits the body and derived title back to
SQLite.

## Update

Single fields `kind`, `status`, `priority`, `scheduled`, and `due` take a value;
use `-` to clear. `home` takes a qualified collection. Set fields `tag`,
`collection`, `link`, and `source` require `+value` or `-value`.

```sh
nt update <id> home work/project_a
nt update <id> collection +personal/rust
nt update <id> tag +decision
```

A home collection cannot be removed until home moves. Terminal statuses manage
the `closed` timestamp.

## Agenda, Remove, And Export

`agenda` includes open and waiting todos and supports `today`, `week`, `overdue`,
`waiting`, and `undated`. `rm` validates all ids and removes notes and dependent
relationships transactionally.

`export <path> [id...]` writes portable `<id>.md` snapshots with generated front
matter. Exported Markdown is not canonical storage.

## Config

`config show` prints the database path. `config vault` prints vault UUID and
name rows. There is no command to select a vault.

## Operation

SQLite transactions provide atomic mutations and foreign-key consistency.
The CLI supports one user-directed writer at a time; SQLite serializes writes,
and commands use a busy timeout. Agents use the same visible interface and
should obtain user approval before mutations.
