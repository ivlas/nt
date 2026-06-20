# nt Design

`nt` is a small CLI-native note organizer for humans and agents. Its product
goal is short, deterministic `time-to-knowledge`: capture a portable note,
narrow candidates with visible filters, retrieve an exact id, and edit the
source directly.

This document records the implemented architecture, accepted constraints, and
deferred decisions. The exact public contract is in
[cli-reference.md](cli-reference.md).

## Boundaries

`nt` is built from plain CommonMark, visible JSON, and Unix process interfaces.
Humans and agents use the same stdin, stdout, `$EDITOR`, files, and commands.

It is not an agent framework, RAG system, vector database, daemon, server,
browser or runtime orchestrator, workflow engine, Codex launcher, or Hermes
replacement. Core behavior must not introduce hidden retrieval, embeddings,
background state, agent-only paths, or runtime management.

The core loop is:

```text
capture -> index -> filter -> inspect -> connect -> revise
```

## Implemented Architecture

The command entry point is `src/main.rs`. `clap` parses the positional grammar
defined in `src/cli.rs`, then `commands::run` routes one typed command. Command
handlers own orchestration: load the index, validate input, read or mutate note
state, persist it, and print a stable result.

Module responsibilities are deliberately narrow:

| Module | Responsibility |
|---|---|
| `cli.rs` | Public command, subcommand, field, view, and shell enums. |
| `commands.rs` | Command routing, validation, vault operations, agenda selection, and mutation rollback. |
| `index.rs` | Serialized metadata, vault state, derived maps, text indexes, and migrations. |
| `query.rs` | Query parsing, candidate planning, and final match verification. |
| `note.rs` | Ids, timestamps, calendar dates, title validation, and URL extraction. |
| `fs.rs` | Paths and atomic temp-file-and-rename writes. |
| `display.rs` | Stable summary and agenda records. |
| `export.rs` | Generated front matter for exported Markdown copies. |
| `completion.rs` | Bash and Zsh completion, including dynamic values. |
| `help.rs` | Flagless built-in help text. |
| `terminal.rs` | TTY-aware ANSI color policy. |
| `error.rs` | Application error types shared across modules. |

Errors propagate as `Result<T, NtError>` to `main`, which prints one red error
line to stderr when color is enabled and exits with status 1. Commands validate
before mutation where possible. `open` and `rm` retain enough prior state to
restore the note file if saving the index fails.

### Command Flow

An `nt add` demonstrates the normal ownership and persistence flow:

1. `clap` produces `Command::Add { metadata }`.
2. `commands::add` loads and owns a mutable `Index`.
3. Creation metadata and the CommonMark title are validated before persistence.
4. `note.rs` allocates an unused UTC id and derives the note path.
5. `fs::atomic_write` writes the Markdown body through a sibling temp file,
   syncs it, renames it, and syncs the parent directory on Unix.
6. `Index::upsert_note_with_body` refreshes body terms and all derived maps.
7. The JSON index is atomically saved. If this fails, the new note is removed.
8. The command prints `saved <id>`.

Reads follow the same explicit route without mutation. For example, `find`
loads the index, parses expressions, intersects available candidate sets,
verifies candidates, and prints matching summaries in active-recent order.

## Storage Decisions

Canonical note bodies live in one configured flat directory as
`NTYYYYMMDDTHHmmss.md`. The filename stem is the id. A note's first non-empty
line is its required `# Title`; the rest is unrestricted CommonMark. Active
notes have no front matter or nt-specific body syntax.

`$HOME/.nt/index.json` stores:

- index version, known vaults, and active vault
- primary note metadata: `id`, `path`, `created`, `updated`, `title`, `kind`,
  `status`, `priority`, `scheduled`, `due`, `closed`, `tags`, `collections`,
  `links`, and `sources`
- rebuildable maps: `recent`, `kinds`, `statuses`, `tags`, `collections`,
  `days`, `backlinks`, and metadata `terms`
- rebuildable `body_terms`, `heading_terms`, and the `body_indexed` trust set

Note bodies are never stored in the index. Metadata that cannot be derived from
CommonMark remains visible primary JSON data and is changed through explicit
commands. Loading an index rebuilds derived metadata maps; `nt rebuild` is the
operation that re-reads active Markdown bodies and refreshes body indexes.

Both note and index writes use temp-file-and-rename. Multi-file mutations cannot
be one filesystem transaction, so command handlers use compensating rollback
for the note file when the following index save fails.

Multiple vaults share one index. Commands operate only on the active vault;
vault names are directory basenames and note ids are globally keyed in the
index.

## Retrieval Decisions

`nt find` is deterministic filtering, not ranked retrieval. All positional
expressions are `AND`-combined and case-insensitive. Exact derived maps narrow
ids for fields such as tags, kinds, statuses, collections, dates, and links.
Metadata and body term indexes narrow text queries. Final matches retain
active-recent order.

Indexed body entries are trusted until `nt rebuild`. This makes normal queries
independent of vault size, but an out-of-band body edit remains stale until the
explicit rebuild. Notes absent from `body_indexed` fall back to direct Markdown
reads. Quoted multiword `body:` values match all indexed terms, not an exact
phrase.

Unknown fields are errors, with a close-field suggestion when available. This
prevents a misspelled structured filter from silently becoming a broad text
search. There is no scoring, fuzzy matching, semantic search, regex, full
Boolean grammar, or public heading query.

## Metadata Decisions

Fields have distinct meanings instead of overloading tags:

- `kind`: structural form; one of `note`, `todo`, `meeting`, `decision`,
  `source`, `research`, or `project`
- `status`: optional action state; `open`, `waiting`, `done`, or `dropped`
- `priority`: optional urgency ordered `S`, `A`, `B`, `C`, `D`
- `scheduled` and `due`: optional validated local calendar dates
- `closed`: system-managed UTC timestamp for a terminal status transition
- `collection`: workspace-like membership; `/` is a naming convention only
- `tag`: sparse reusable topic or entity
- `link`: exact outbound note relationship stored in JSON
- `source`: external reference supplied explicitly or extracted from a body URL

Set-like changes require an explicit add or remove operation. This keeps
updates idempotent and makes an agent's intended mutation reviewable. Links do
not require wiki syntax; active note bodies remain portable CommonMark.

Agenda behavior is also fixed: only open or waiting `todo` notes are actionable,
each appears in exactly one default section, and ordering is date, then priority,
then active recency. Terminal status transitions manage `closed`; users cannot
set it directly.

Tags should remain sparse. Inspect `nt list tags`, prefer an existing lowercase
tag, use one to three by default, and add a new tag only for a recurring concept.
Collections represent durable groups such as `projects/nt` or `research/qemu`.

## Interface Decisions

Core workflows are flagless. Commands prefer positional arguments, stdin,
stdout, and `$EDITOR`. Machine-facing projections are one record per line;
mutations print one short lowercase status line. Summary records keep the id
visually dominant, and paths are relative to the current directory when
possible.

ANSI color is limited to TTY output and disabled for pipes, `NO_COLOR`, or
`TERM=dumb`. Paging, fuzzy selection, previews, and batching belong to `less`,
`fzf`, `awk`, `xargs`, and similar tools. A TUI is intentionally deferred and
is not part of the current core.

Agents use the same interface. `nt` does not launch agents, install skills,
generate agent workspaces, or keep hidden memory. Agent-driven writes should be
drafted and approved before `nt add`, `$EDITOR`, or `nt update` mutates state.

## Decision Status

The original command-surface plan is complete in the current code: `list`
projections, typed `update`, agenda fields and views, dynamic completion, and
legacy command removal are implemented and covered by tests. The 0.1.0 stable
core is therefore the current storage, retrieval, metadata, and shell contract,
not a pending migration.

Future changes are constrained to preserve canonical CommonMark, visible JSON,
explicit commands, deterministic output, atomic writes, and no hidden runtime.
The following ideas are deliberately deferred rather than promised:

- a public `heading:<term>` query; `heading_terms` exists only as an internal
  rebuildable index
- `OR` or grouping; add it only if real use requires it and keep grouping
  explicit
- recurrence, effort estimates, time tracking, habits, or Markdown task parsing
- a TUI or broader browse, graph, workflow, or runtime commands
- richer workspace, research queue, import, tag, and completion workflows

## Development And Release

Behavior changes should add focused parser, storage, query, completion, and
command tests as appropriate. Before release, run:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets
cargo run -- help
cargo run -- help find
cargo run -- help rebuild
```

Also run the README quick start manually, verify that documented commands are
implemented, and ensure no local note or index files are committed.
