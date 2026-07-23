# nt Design

`nt` is a small CLI-native note organizer for its user. Its product goal
is short, deterministic `time-to-knowledge`: capture a portable note, narrow
candidates with visible filters, retrieve an exact id, and edit the source
directly.

This document records the implemented architecture, accepted constraints, and
deferred decisions. The exact public contract is in
[cli-reference.md](cli-reference.md).

## Boundaries

`nt` is built from plain CommonMark, visible JSON, and Unix process interfaces.
The user owns the notes and decides every mutation. They can use the same
stdin, stdout, `$EDITOR`, files, and commands directly or direct an agent to do
so. An agent has no autonomous note-taking or mutation path.

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
defined in `src/cli/mod.rs`, then `commands::run` routes one typed command.
Command handlers own orchestration: load the index, validate input, read or
mutate note state, persist it, and print a stable result.

Module responsibilities are deliberately narrow. The source tree groups
related concerns into directories, each with a `mod.rs` entry point:

| Module | Responsibility |
|---|---|
| `cli/mod.rs` | Public command, subcommand, field, view, and shell enums. |
| `cli/help.rs` | Flagless built-in help text. |
| `cli/completion.rs` | Bash and Zsh completion script generation, including dynamic values. |
| `commands/mod.rs` | Command routing, shared validators, status transitions, and index helpers. |
| `commands/init.rs` | `init` and Markdown import for existing flat vaults. |
| `commands/add.rs` | `note`/`todo`, creation metadata parsing, and editor plumbing. |
| `commands/show.rs` | `show`, `open`, and `find`. |
| `commands/rm.rs` | `rm` and index removal. |
| `commands/update.rs` | `update` and the update operation model. |
| `commands/list.rs` | `list` orchestration and link graph rendering. |
| `commands/agenda.rs` | `agenda` sections, selection, and ordering. |
| `commands/export_cmd.rs` | `export` and active-vault guards. |
| `commands/config.rs` | `config show` and `config vault`. |
| `index/mod.rs` | Serialized primary metadata, vault state, and persistence. |
| `listing/mod.rs` | List request parsing, compatibility forms, and filter dispatch. |
| `listing/field.rs` | `ListField` enum, projection parsing, and per-field rendering. |
| `listing/render.rs` | Row and table layout for TTY and pipe output. |
| `query/mod.rs` | `Query` and `QueryExpr` types, public parse/match API. |
| `query/parse.rs` | Expression parsing, field validators, and unknown-field suggestions. |
| `query/eval.rs` | Metadata matching and on-demand body reads. |
| `query/suggest.rs` | Edit-distance field suggestion utility. |
| `note/id.rs` | Note id validation, id-to-iso conversion, and collision-safe id allocation. |
| `note/date.rs` | Timestamps, calendar date validation, and date arithmetic. |
| `note/body.rs` | Title extraction and URL source extraction from CommonMark bodies. |
| `fs/paths.rs` | Home and nt-home resolution, index path, and cwd-relative paths. |
| `fs/atomic.rs` | Atomic temp-file-and-rename writes. |
| `display.rs` | Stable summary and agenda records. |
| `export.rs` | Generated front matter for exported Markdown copies. |
| `terminal.rs` | TTY-aware ANSI color policy. |
| `error.rs` | Application error types shared across modules. |

Errors propagate as `Result<T, NtError>` to `main`, which prints one red error
line to stderr when color is enabled and exits with status 1. Commands validate
before mutation where possible. A mutation that fails after changing a file
leaves that visible state in place for the user to inspect.

### Command Flow

An `nt note` demonstrates the normal ownership and persistence flow:

1. `clap` produces `Command::Note { metadata }`.
2. `commands::add::note` loads and owns a mutable `Index`.
3. Creation metadata and the CommonMark title are validated before persistence.
4. `note::id` allocates an unused UTC id and derives the note path.
5. `fs::atomic_write` writes the Markdown body through a sibling temp file,
   syncs it, then renames it into place.
6. `Index::upsert_note` stores the primary metadata record.
7. The JSON index is atomically saved. If this fails, the new note remains a
   visible file for the user to inspect.
8. The command prints `saved <id>`.

Reads follow the same explicit route without mutation. For example, `find`
loads the index, parses expressions, evaluates them against every active note's
metadata, reads Markdown bodies from disk for body terms, and prints matching
summaries in newest-first order. `list` evaluates the same structured filters,
then projects the requested `NoteMeta` fields into tab-separated rows.

## Storage Decisions

Canonical note bodies live in one configured flat directory as
`NTYYYYMMDDTHHmmss.md`. The filename stem is the id. A note's first non-empty
line is its required `# Title`; the rest is unrestricted CommonMark. Active
notes have no front matter or nt-specific body syntax.

`$HOME/.nt/index.json` stores only primary state:

- index version, known vaults, and active vault
- primary note metadata: `id`, `path`, `created`, `updated`, `title`, `kind`,
  `status`, `priority`, `scheduled`, `due`, `closed`, `tags`, `collections`,
  `links`, and `sources`

Note bodies are never stored in the index, and no derived maps are persisted:
ordering, filtering, and body matching are computed at query time from primary
metadata and the Markdown files themselves. There is no cache to invalidate
and no rebuild command. Metadata that cannot be derived from CommonMark
remains visible primary JSON data and is changed through explicit commands.

Both note and index writes use temp-file-and-rename. Multi-file mutations are
not filesystem transactions: a failed later write can leave visible note and
index state for the user to inspect. This is an intentional trade for a small,
user-directed, single-writer CLI rather than a hidden coordination layer.

Multiple vaults share one index. Commands operate only on the active vault;
vault names are directory basenames and note ids are globally keyed in the
index.

## Retrieval Decisions

`nt find` is deterministic filtering, not ranked retrieval. All positional
expressions are `AND`-combined and case-insensitive. Every expression is
evaluated directly against each active note's primary metadata; `body:` terms
and unmatched bare words read the note's Markdown file at query time. Body
search reads Markdown bodies at query time, so out-of-band edits are visible
to search immediately. Quoted multiword `body:` values match all terms, not an
exact phrase. Final matches retain newest-first order.

Scanning every active note keeps retrieval correct by construction and stays
fast for vaults in the tens of thousands of notes: metadata fits in a few
megabytes of JSON, and body reads only happen for textual expressions. If body
scans ever become the bottleneck, reach for `memchr`-based substring search or
parallel file reads first; either speeds up scanning without reintroducing a
persisted text index.

Unknown fields are errors, with a close-field suggestion when available. This
prevents a misspelled structured filter from silently becoming a broad text
search. There is no scoring, fuzzy matching, semantic search, regex, full
Boolean grammar, or public heading query.

Structured selection is shared with `nt list`. List accepts exact metadata and
created-date expressions, including `not`, but rejects bare words and `title`,
`source`, and `body` search expressions. This keeps list responsible for
metadata inspection and projection while find remains the textual retrieval
command. Both evaluate the same `Query` type and retain newest-first order.

## Metadata Decisions

Fields have distinct meanings instead of overloading tags:

- `kind`: system shape; one of `note` or `todo`
- `status`: optional todo action state; `open`, `waiting`, `done`, or `dropped`
- `priority`: optional todo urgency ordered `S`, `A`, `B`, `C`, `D`
- `scheduled` and `due`: optional todo calendar dates
- `closed`: system-managed UTC timestamp for a terminal status transition
- `collection`: workspace-like membership; `/` is a naming convention only
- `tag`: sparse reusable topic or entity
- `link`: exact outbound note relationship stored in JSON
- `source`: external reference supplied explicitly or extracted from a body URL

Set-like changes require an explicit add or remove operation. This keeps
updates idempotent and makes a user-directed mutation reviewable. Links do not
require wiki syntax; active note bodies remain portable CommonMark.

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

Mutation commands assume one user-directed writer at a time; concurrent
mutations are outside the supported workflow. Read-only commands remain safe to
run freely. The user can perform writes directly or ask an agent to perform a
specific, approved write, but an agent never decides to mutate the vault alone.

List projections use a comma-separated field argument. Interactive output has
a header and aligned columns; redirected output is headerless and tab-separated.
Bare `nt list` expands to the fixed summary `id`, `title`, `kind`, `status`,
`due`, and `tag`; `nt list all` expands to every metadata field. Scripts
should request explicit fields, such as
`nt list id,title,status`, so defaults can evolve without changing their column
positions. Plural `tags`, `collections`, and `links` remain explicit metadata
vocabulary and relationship operations.

ANSI color is limited to TTY output and disabled for pipes, `NO_COLOR`, or
`TERM=dumb`. Paging, fuzzy selection, previews, and batching belong to `less`,
`fzf`, `awk`, `xargs`, and similar tools. A TUI is intentionally deferred and
is not part of the current core.

Agents use the same interface only on the user's direction. `nt` does not
launch agents, install skills, generate agent workspaces, keep hidden memory, or
provide an agent-owned note workflow. Agent-driven writes should be drafted and
approved before `nt note`, `$EDITOR`, or `nt update` mutates state.

## Decision Status

The command surface includes generic list projections and structured filters,
typed `update`, agenda fields and views, dynamic completion, and compatibility
forms for the original list submodes. The 0.1.0 stable
core remains the current storage, retrieval, metadata, and shell contract, not
a pending storage migration.

Future changes are constrained to preserve canonical CommonMark, visible JSON,
explicit commands, deterministic output, atomic writes, and no hidden runtime.
The following ideas are deliberately deferred rather than promised:

- a public `heading:<term>` query
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
```

Also run the README quick start manually, verify that documented commands are
implemented, and ensure no local note or index files are committed.
