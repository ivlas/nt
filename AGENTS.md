# AGENTS.md

## Project

`nt` is a local, agent-first application for editable CommonMark notes and
immutable memory, with deterministic metadata and retrieval. It prioritizes
fast capture, bounded chronological waking, low token use, and local ownership.

Humans and agents use the same Unix-like interface. Commands read stdin, write
stdout, use `$VISUAL`/`$EDITOR`, and compose with normal shell tools. User
approval is required before agent-driven mutations.

## Rules

- The binary name is `nt` and core workflows are flagless and configless.
- `nt` has two separate first-class models: notes for editable durable knowledge
  and memory for immutable durable experience and history. Do not merge their
  schemas or repositories.
- Canonical state lives in `$HOME/.nt/nt.sqlite3`.
- Store CommonMark note bodies directly in SQLite.
- Store raw memory bodies directly in SQLite as one contiguous, zero-based,
  append-only sequence. Raw memories are canonical, immutable single lines of
  at most 512 Unicode characters.
- Use canonical lowercase UUIDv7 IDs for notes.
- Use non-negative integer sequence numbers as public memory identities.
- Every note has exactly one normalized collection path; `inbox` is the default.
- Tags are optional, and links are explicit and directional.
- There are no vaults, collection entities, note kinds, todos, sources, generic
  metadata, additional memberships, or active namespace state.
- Represent external resources, bookmarks, imported documents, and generated
  summaries as ordinary CommonMark notes with collections, tags, and links.
  Do not assign them reserved kinds or hidden semantics.
- There is no canonical Markdown vault, JSON index, or rebuild workflow.
- Memory summaries are derived, disposable state over aligned binary half-open
  ranges. Store only `(lo, hi, body)`; calculate tree relationships.
- Use SQLite transactions and foreign keys for consistency.
- Do not add wiki-link syntax or nt-specific note-body markup.
- Do not add hidden agent-only behavior or built-in agent launchers.
- Do not create storage from ordinary commands; only `nt init` may initialize it.
- Use `clap`, `rusqlite`, `uuid`, `serde_json`, and `thiserror` for their
  established responsibilities.
- Do not add memory FTS, embeddings, vector databases, automatic model calls,
  work queues, background workers, daemons, or generic domain abstractions.
- Memory limits are fixed compile-time constants: raw entries and summaries are
  at most 512 Unicode characters, and `WAKE_ENTRIES = 128` is the only wake
  knob.

## Commands

The canonical command contract is `docs/cli-reference.md`:

- `nt init`
- `nt add [metadata...] [-- body...]`
- `nt show <id>`
- `nt list [filter...]|tags|collections`
- `nt find <term-or-filter...>`
- `nt rm <id...>`
- `nt edit <id> [-- body...]`
- `nt move <id> <collection>`
- `nt tag <id> <+tag|-tag>`
- `nt link <id> <+id|-id>`
- `nt memory add [memory...]`
- `nt memory wake`
- `nt memory recall <pattern...>`
- `nt memory nap`
- `nt memory nap <range> [summary...]`
- `nt memory zoom <range>`
- `nt memory forget <range>`
- `nt help [command...]`

Avoid broader commands or hidden runtime management until concrete usage proves
the need.

## Storage

Primary relational state consists of notes, tags, directional note links, and
raw memories. The note row stores its collection path, canonical body, derived
title, timestamps, and body version. The raw memory row stores its sequence,
canonical body, and creation timestamp. Note FTS and memory summaries are
derived state maintained transactionally.

The memory schema contains exactly `memory(sequence, created_at, body)` and
`memory_summary(lo, hi, body)`. Summary ranges are aligned, binary, and
half-open internally; the CLI renders them inclusively, such as `0-1` and
`0-7`. Each summary has two direct children. `nap` derives the smallest
buildable missing range, and raw insertion never waits for summary creation.

The clean-sheet schema has no migration system. The database uses application
ID `0x4e544e54` (`NTNT`). Enable foreign keys on every operational connection
and use WAL with a bounded busy timeout. The flat, compile-time manifest
explicitly contains application, note, and memory schema objects.

Markdown exists at the interface boundary: trailing arguments, stdin,
`$VISUAL`/`$EDITOR`, exact display, and later explicitly approved exports.

## Retrieval

- Use `nt list` and structured filters for cheap candidate construction.
- Use `nt find` for deterministic metadata and literal FTS filtering.
- Use `nt show <id>` only for exact body retrieval.
- Keep output stable and one record per line.
- Redirected list and find output is JSON-encoded, headerless TSV.
- List and find are complete by default; `limit:` is explicit and SQL-backed.
- Stream redirected note summaries and stop cleanly when a downstream pipe closes.
- Unknown query fields are errors.
- Avoid scoring, fuzzy matching, embeddings, vector search, and hidden retrieval.
- Use `nt memory recall` for a case-sensitive literal substring scan over raw
  history. It returns matching raw entries chronologically.
- Use `nt memory wake` for a deterministic chronological age-decaying dyadic
  cover. It returns all raw entries when history fits `WAKE_ENTRIES` and requires
  every derived summary selected for larger history.
- Use `nt memory nap` to obtain the smallest buildable summary range, `zoom` to
  reveal two direct children, and `forget` to remove a summary and its ancestors.
- Memory retrieval has no FTS, scoring, tokenization, stemming, fuzzy matching,
  embeddings, or hidden semantic behavior.

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
  immutable zero-based memory, single-line limits, binary range arithmetic,
  smallest-buildable summary selection, wake covers, literal recall, zoom,
  forget, missing summaries, and concurrent append.

## Design

`docs/design.md` is the detailed clean-sheet contract. Notes and memory remain
separate concrete product models. Memory must not be represented as reserved
note kinds, collections, tags, generic metadata, or hidden note semantics. Do
not introduce registries, repository traits, plugins, dependency injection, or
generic note/memory entities.

## Commits

Use concise `fix:`, `refactor:`, `chore:`, `docs:`, or `test:` prefixes and keep
commits focused.
