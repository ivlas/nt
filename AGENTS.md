# AGENTS.md

## Project

`nt` is a local, agent-first knowledge and memory layer. It prioritizes fast
capture, deterministic retrieval, bounded context construction, durable logical
organization, low LLM token use, and local ownership.

Humans and agents use the same Unix-like interface. Commands read stdin, write
stdout, use `$EDITOR`, and compose with normal shell tools. User approval is
required before agent-driven mutations.

## Rules

- The binary name is `nt` and core workflows are flagless.
- The tool should not have config: do not add configuration files or a `config` command.
- Canonical state lives in `$HOME/.nt/nt.sqlite3`.
- Store CommonMark note bodies directly in SQLite.
- Use canonical lowercase UUIDv7 ids for notes, vaults, and collections.
- Vaults are logical namespaces, never filesystem directories.
- Collections belong to exactly one vault and use `<vault>/<collection>` names.
- Notes use many-to-many collection memberships and exactly one home collection.
- The home collection must also be a membership.
- A note may reference collections from multiple vaults.
- There is no active-vault state, JSON index, canonical Markdown vault, or
  rebuild workflow.
- Use SQLite transactions and foreign keys for consistency.
- Do not add wiki-link syntax or nt-specific note-body markup.
- Do not add hidden agent-only behavior or built-in agent launchers.
- Use `clap`, `rusqlite`, `uuid`, `serde_json`, and `thiserror` for their
  established responsibilities.

## Commands

The canonical command contract is `docs/cli-reference.md`:

- `nt init <vault>`
- `nt note [metadata...]`
- `nt todo [metadata...]`
- `nt list [projection] [filter...]`
- `nt find <expr...>`
- `nt show <id>`
- `nt rm <id...>`
- `nt update <id> <field> <value>`
- `nt agenda [week]`
- `nt export <path> [id...]`
- `nt help [command...]`

Avoid broader commands or hidden runtime management until concrete usage proves
the need.

## Storage

Primary relational state includes vaults, vault-owned collections, notes with
bodies, note-collection memberships, tags, links, and sources. Persist only
primary state. Enable foreign keys on every connection and keep schema changes
versioned.

`home_collection_id` is canonical placement. Other `note_collections` rows are
references. Moving home may preserve the old membership until explicitly
removed. Never permit removal of the current home membership.

Markdown exists at the interface boundary: stdin, `$EDITOR`, display, and
export. Export files are snapshots, not canonical storage.

## Retrieval

- Use `nt list` and explicit projections for cheap candidate construction.
- Use `nt find` for deterministic metadata/body filtering.
- Use `nt show <id>` only for exact body retrieval.
- Keep output stable, grep-friendly, and one record per line.
- Unknown query fields are errors.
- Avoid scoring, fuzzy matching, embeddings, vector search, and hidden retrieval
  unless a separately approved design requires them.

## Metadata

- `kind`: `note` or `todo`
- `status`: `open`, `waiting`, `done`, or `dropped`
- `priority`: `S`, `A`, `B`, `C`, or `D`
- `scheduled`, `due`: `YYYY-MM-DD`
- `closed`: system-managed terminal transition timestamp
- `home`: canonical `<vault>/<collection>`
- `collection`: additional qualified membership
- `tag`: sparse topic or entity
- `link`: exact note UUID relationship
- `source`: external reference

## Terminal UX

- Successful mutations print one short lowercase line.
- Keep ids visually dominant.
- Use ANSI color only on TTY stdout and respect `NO_COLOR` and `TERM=dumb`.
- Avoid banners, boxes, spinners, and progress bars.

## Testing

- Run `cargo fmt` before finishing Rust changes.
- Run `cargo test` when behavior changes.
- Run `cargo run -- help` for a command smoke test.
- Add focused tests for schema constraints, transactions, UUIDv7 ids, command
  routing, query syntax, metadata updates, and cross-vault memberships.

## Commits

Use concise `fix:`, `refactor:`, `chore:`, `docs:`, or `test:` prefixes and keep
commits focused.
