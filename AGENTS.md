# AGENTS.md

## Project

`nt` is a local, agent-first note layer for editable CommonMark notes,
deterministic metadata, and lexical retrieval. It prioritizes fast capture,
bounded context construction, low LLM token use, and local ownership.

Humans and agents use the same Unix-like interface. Commands read stdin, write
stdout, use `$EDITOR`, and compose with normal shell tools. User approval is
required before agent-driven mutations.

## Rules

- The binary name is `nt` and core workflows are flagless and configless.
- Canonical state lives in `$HOME/.nt/nt.sqlite3`.
- Store CommonMark note bodies directly in SQLite.
- Use canonical lowercase UUIDv7 IDs for notes.
- Every note has exactly one normalized collection path; `inbox` is the default.
- Tags are optional, and links are explicit and directional.
- There are no vaults, collection entities, note kinds, todos, sources, generic
  metadata, additional memberships, or active namespace state.
- There is no canonical Markdown vault, JSON index, or rebuild workflow.
- Use SQLite transactions and foreign keys for consistency.
- Do not add wiki-link syntax or nt-specific note-body markup.
- Do not add hidden agent-only behavior or built-in agent launchers.
- Do not create storage from ordinary commands; only `nt init` may initialize it.
- Use `clap`, `rusqlite`, `uuid`, `serde_json`, and `thiserror` for their
  established responsibilities.

## Commands

The canonical command contract is `docs/cli-reference.md`:

- `nt init`
- `nt add [metadata...] [-- body...]`
- `nt show <id>`
- `nt list [filter...]`
- `nt find <term-or-filter...>`
- `nt rm <id...>`
- `nt edit <id> [-- body...]`
- `nt move <id> <collection>`
- `nt tag <id> <+tag|-tag>`
- `nt link <id> <+id|-id>`
- `nt help [command...]`

Avoid broader commands or hidden runtime management until concrete usage proves
the need.

## Storage

Primary relational state consists of notes, tags, and directional note links.
The note row stores its collection path, canonical body, derived title,
timestamps, and body version. Persist only primary state. FTS5 is derived state
maintained transactionally by SQLite triggers.

The clean-sheet schema is version `1`, with no migration compatibility for the
old development schema. The database uses application ID `0x4e544e54` (`NTNT`).
Enable foreign keys on every operational connection and use WAL with a bounded
busy timeout.

Markdown exists at the interface boundary: trailing arguments, stdin,
`$EDITOR`, exact display, and later explicitly approved exports.

## Retrieval

- Use `nt list` and structured filters for cheap candidate construction.
- Use `nt find` for deterministic metadata and literal FTS filtering.
- Use `nt show <id>` only for exact body retrieval.
- Keep output stable and one record per line.
- Redirected list and find output is JSON-encoded, headerless TSV.
- Unknown query fields are errors.
- Avoid scoring, fuzzy matching, embeddings, vector search, and hidden retrieval.

## Terminal UX

- Successful mutations print one short lowercase line.
- Keep IDs visually dominant.
- Use ANSI color only on TTY stdout and respect `NO_COLOR` and `TERM=dumb`.
- Avoid banners, boxes, spinners, and progress bars.

## Testing

- Run `cargo fmt` before finishing Rust changes.
- Run `cargo test` when behavior changes.
- Run `cargo run -- help` for a command smoke test.
- Add focused tests for database identity, schema constraints, transactions,
  UUIDv7 IDs, command routing, query syntax, body conflicts, and link cleanup.

## Design

`docs/design.md` is the detailed clean-sheet contract. Append-only memory
requires a separate RFC and must not be represented as a note kind, collection,
or tag.

## Commits

Use concise `fix:`, `refactor:`, `chore:`, `docs:`, or `test:` prefixes and keep
commits focused.
