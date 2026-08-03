# nt CLI Reference

`nt` is a flagless, configless local CLI over `$HOME/.nt/nt.sqlite3`. SQLite is
canonical for note bodies, metadata, vaults, collections, and relationships.

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
nt agenda [week]
nt export <path> [id...]
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

Capture, home movement, and `collection +...` create an unknown collection when
its vault exists. Reads never create collections, and unknown vaults are errors.
Move home with `nt update <id> home <vault>/<collection>`. The old home remains a
reference until removed with a collection update.

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
```

Fields are `id`, `home`, `created`, `updated`, `title`, `kind`, `status`,
`priority`, `scheduled`, `due`, `closed`, `tag`, `collection`, `link`, and
`source`; plain `nt list` selects `id,title,kind,collection,tag`, and `all`
selects every field. Redirected projections are headerless
tab-separated rows, one per note. Set-valued fields are comma-separated. `list`
accepts AND-combined structured metadata filters only; bare words and `title:`,
`source:`, and `body:` filters require `find`.

`find` field names are lowercase and expressions are AND-combined:

```text
<word> #<tag> id:<prefix> tag:<tag> title:<term>
day:<date> since:<date> before:<date> kind:<kind> status:<status>
priority:<priority> scheduled:<date> due:<date> closed:<date>
collection:<vault>/<collection> link:<id> source:<term> body:<term> not:<expr>
```

Bare, `title:`, and `body:` values use the FTS5 `unicode61` tokenizer. `nt`
splits each value into unique Unicode letter-or-digit runs; punctuation,
including quotes, hyphens, slashes, and `*`, separates terms. Every term must
match a complete token, but terms may occur in any order. Shell-quoted
multiword values are therefore AND searches, not phrase searches. Bare terms
may match across title and body. Unicode case is folded and diacritics are
removed from Latin characters where supported by `unicode61`, so `cafe` matches
`café`. Prefix matching is not enabled: `stor` and `stor*` do not match
`storage`. A `title:` or `body:` value containing only punctuation matches
nothing.

Bare values also retain ASCII case-insensitive literal substring matching over
non-text metadata, including sources. `source:` remains an ordinary SQL
substring predicate rather than an FTS field. Structured metadata filters use
ordinary SQL predicates and their existing ASCII normalization. Retrieval is
deterministic, ordered by creation time and id rather than relevance, and
unranked; there is no scoring, fuzzy search, embeddings, or semantic search.

## Show And Open

`show` prints id, title, home, timestamps, kind-specific metadata, then the exact
body. Note output omits the todo-only `status`, `priority`, `scheduled`, `due`,
and `closed` fields. `open` copies the body to a temporary file for `$EDITOR`,
without holding a database transaction open. Saving atomically requires the
original update timestamp and body; otherwise `open` reports that the note
changed during editing. The body, derived title, timestamp, and body-derived
sources are committed in one transaction.

## Update

`kind` takes `note` or `todo`; `kind -` resets it to `note`. `status`,
`priority`, `scheduled`, and `due` use `-` to clear. `home` takes a qualified
collection. Set fields `tag`, `collection`, `link`, and `source` require
`+value` or `-value`.

```sh
nt update <id> home work/project_a
nt update <id> collection +personal/rust
nt update <id> tag +decision
```

A home collection cannot be removed until home moves. Terminal statuses manage
the `closed` timestamp.

## Agenda, Remove, And Export

`agenda` is a date-focused view of open todos. By default it includes overdue
todos and todos due today or scheduled today or earlier. `agenda week` also
includes todos scheduled or due in the next six days. Waiting, undated, done,
and dropped todos are excluded. Rows contain tab-separated `id`, `priority`,
`scheduled`, `due`, and `title` fields and are ordered by date, priority, and
recency. `rm` rejects duplicate ids and verifies that every requested note
exists inside one transaction before deleting anything. A missing id leaves
every note unchanged. Cascades remove memberships, tags, sources, and incoming
and outgoing links.

`export <path> [id...]` writes portable `<id>.md` snapshots with generated front
matter. `<path>` is a directory and is created if necessary. Omitting ids
exports all notes; explicit ids are validated and deduplicated before any
snapshot is written. Each file is atomically replaced, but a multi-file export
is not transactional. Exported Markdown is not canonical storage.

## Operation

SQLite transactions provide atomic database mutations and foreign-key
consistency. WAL mode lets readers continue from the last committed snapshot
while another connection writes. Independent commands may write concurrently,
but SQLite remains a single-writer database: a contending writer waits for the
five-second busy timeout and then reports a retryable error. Transactions are
short and never remain open while `nt` reads note content from stdin or waits for
`$EDITOR`; stale editor saves are rejected. Agents use the same visible
interface and should obtain user approval before mutations.
