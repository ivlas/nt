# AGENTS.md

## Project

`nt` is a local, agent-first application for editable CommonMark notes with
deterministic metadata and lexical retrieval. It prioritizes fast capture, low
LLM token use, and local ownership.

Humans and agents use the same Unix-like interface. Commands read stdin, write
stdout, use `$VISUAL`/`$EDITOR`, and compose with normal shell tools. User
approval is required before agent-driven mutations.

## Rules

- The binary name is `nt` and core workflows are flagless and configless.
- Notes are the only product model in this repository and CLI.
- Canonical state lives in `$HOME/.nt/nt.sqlite3`.
- Store CommonMark note bodies directly in SQLite.
- Use canonical lowercase UUIDv7 IDs for notes.
- Every note has exactly one normalized collection path; `inbox` is the default.
- Tags are optional, and links are explicit and directional.
- There are no vaults, collection entities, note kinds, todos, sources, generic
  metadata, additional memberships, or active namespace state.
- Represent external resources, bookmarks, imported documents, and generated
  summaries as ordinary CommonMark notes with collections, tags, and links.
  Do not assign them reserved kinds or hidden semantics.
- There is no canonical Markdown vault, JSON index, or rebuild workflow.
- Use SQLite transactions and foreign keys for consistency.
- Do not add wiki-link syntax or nt-specific note-body markup.
- Do not add hidden agent-only behavior or built-in agent launchers.
- Do not create storage from ordinary commands; only `nt init` may initialize it.
- Use `clap`, `rusqlite`, `uuid`, `serde_json`, and `thiserror` for their
  established responsibilities.
- Do not add other product models, embeddings, vector databases, automatic
  summarization calls, background workers, daemons, or generic domain
  abstractions.

## Commands

The canonical command contract is `docs/cli-reference.md`:

- `nt init`
- `nt add [metadata...] [-- body...]`
- `nt show <id>`
- `nt list [filter...]|tags|collections`
- `nt read [filter...]`
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
timestamps, and body version. Note FTS5 is derived state maintained
transactionally.

The clean-sheet schema follows the current alpha compatibility policy rather
than a general migration system. The database uses application ID `0x4e544e54`
(`NTNT`). Enable foreign keys on every operational connection and use WAL with a
bounded busy timeout. The flat, compile-time manifest explicitly contains
application and note schema objects.

Markdown exists at the interface boundary: trailing arguments, stdin,
`$VISUAL`/`$EDITOR`, exact display, and later explicitly approved exports.

## Retrieval

- Use `nt list` and structured filters for cheap candidate construction.
- Use `nt find` for deterministic metadata and literal FTS filtering.
- Use `nt show <id>` for one exact body and `nt read` for complete filtered notes.
- Keep output stable and one record per line.
- Redirected list and find output is JSON-encoded, headerless TSV.
- Redirected read output is JSONL with complete note records.
- List, read, and find are complete by default; `limit:` is explicit and SQL-backed.
- Stream redirected results and stop cleanly when a downstream pipe closes.
- Unknown query fields are errors.
- Avoid scoring, fuzzy matching, embeddings, vector search, and hidden retrieval.

## Terminal UX

- Successful mutations print one short lowercase line.
- Keep IDs visually dominant.
- Use ANSI color only on TTY stdout and respect `NO_COLOR` and `TERM=dumb`.
- Avoid banners, boxes, spinners, and progress bars.

## Testing

- Pass `--locked` to every Cargo command that supports it.
- Run `cargo fmt` before finishing Rust changes.
- Run `cargo test --locked` when behavior changes.
- Run `cargo run --locked -- help` for a command smoke test.
- Add focused tests for database identity, schema constraints, transactions,
  UUIDv7 IDs, command routing, query syntax, body conflicts, link cleanup,
  FTS synchronization, concurrent edits, and deterministic retrieval.
- Maintain ignored scale coverage for important note query plans.

## Design

`docs/design.md` is the detailed clean-sheet contract. Do not introduce
registries, repository traits, plugins, dependency injection, or generic domain
entities.

## Commits

Use concise `fix:`, `refactor:`, `chore:`, `docs:`, or `test:` prefixes and keep
commits focused.
