# AGENTS.md

## Project

`nt` is a local, agent-first application for editable CommonMark notes and
immutable memory, with deterministic metadata and lexical retrieval. It
prioritizes fast capture, bounded context construction, low LLM token use, and
local ownership.

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
- Store raw memory bodies directly in SQLite as one monotonically ordered,
  append-only sequence. Raw memories are canonical and immutable.
- Use canonical lowercase UUIDv7 IDs for notes.
- Use SQLite-assigned integer sequence numbers as public memory identities.
- Every note has exactly one normalized collection path; `inbox` is the default.
- Tags are optional, and links are explicit and directional.
- There are no vaults, collection entities, note kinds, todos, sources, generic
  metadata, additional memberships, or active namespace state.
- Represent external resources, bookmarks, imported documents, and generated
  summaries as ordinary CommonMark notes with collections, tags, and links.
  Do not assign them reserved kinds or hidden semantics.
- There is no canonical Markdown vault, JSON index, or rebuild workflow.
- Memory summaries, memory FTS indexes, and pending-summary jobs are derived,
  disposable state that must be reconstructable from raw memories.
- Use SQLite transactions and foreign keys for consistency.
- Do not add wiki-link syntax or nt-specific note-body markup.
- Do not add hidden agent-only behavior or built-in agent launchers.
- Do not create storage from ordinary commands; only `nt init` may initialize it.
- Use `clap`, `rusqlite`, `uuid`, `serde_json`, and `thiserror` for their
  established responsibilities.
- Do not add embeddings, vector databases, automatic summarization calls,
  background workers, daemons, or generic domain abstractions.
- Memory v1 limits are fixed compile-time constants: entries and summaries are
  at most 1,024 Unicode characters, context stdout contains at most 32,768
  Unicode characters including metadata and formatting, and summary fanout is
  16.

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
- `nt memory add [-- body...]`
- `nt memory show <seq|node>`
- `nt memory list [filter...]`
- `nt memory recall <term-or-filter...>`
- `nt memory context [term...]`
- `nt memory pending [node|limit:N]`
- `nt memory summarize <node> [-- summary...]`
- `nt memory expand <node>`
- `nt memory invalidate <node>`
- `nt memory status`
- `nt help [command...]`

Avoid broader commands or hidden runtime management until concrete usage proves
the need.

## Storage

Primary relational state consists of notes, tags, directional note links, and
raw memories. The note row stores its collection path, canonical body, derived
title, timestamps, and body version. The raw memory row stores its sequence,
canonical body, and creation timestamp. FTS5 and memory summaries are derived
state maintained transactionally.

Memory summaries form a fixed 16-way pyramid. Node `L0:B` covers 16 raw
memories; each higher node covers 16 summaries from the preceding level. Nodes
are identified by `(level, block)`, and parent, child, and range relationships
are calculated with checked integer arithmetic rather than persisted pointers.
Summarization is delegated to the calling agent through explicit pending and
summarize commands; appending never waits for summarization.

The clean-sheet schema follows the current alpha compatibility policy rather
than a general migration system. The database uses application ID `0x4e544e54`
(`NTNT`). Enable foreign keys on every operational connection and use WAL with a
bounded busy timeout. The flat, compile-time manifest explicitly contains
application, note, and memory schema objects.

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
- Use `nt memory recall` for exact raw historical search through literal-token
  FTS, `nt memory context` for deterministic bounded context compilation, and
  `nt memory expand` for progressive recovery of exact raw history.
- Every memory-context candidate query is SQL-bounded. Never truncate an item to
  fit the 32 KiB output budget; skip candidates that do not fit.
- Prefer exact raw evidence over overlapping summaries, avoid redundant summary
  ranges, and render selected context items chronologically.

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
  immutable memory, tree arithmetic, summary jobs, FTS synchronization,
  invalidation, expansion, concurrent append, and bounded deterministic context.
- Maintain an ignored scale fixture that can populate one million raw memories
  and measure append, show, range listing, recall, context, expansion, pending
  work, and database size.

## Design

`docs/design.md` is the detailed clean-sheet contract. Notes and memory remain
separate concrete product models. Memory must not be represented as reserved
note kinds, collections, tags, generic metadata, or hidden note semantics. Do
not introduce registries, repository traits, plugins, dependency injection, or
generic note/memory entities.

## Commits

Use concise `fix:`, `refactor:`, `chore:`, `docs:`, or `test:` prefixes and keep
commits focused.
