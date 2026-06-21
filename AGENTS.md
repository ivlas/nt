# AGENTS.md

## Project

`nt` is a small CLI-native note organizer for humans and agents. It is built
around visible commands, deterministic retrieval, editable CommonMark notes, and
no hidden memory layer.

Humans and agents use the same Unix-like interface. Commands should read stdin,
write stdout, use `$EDITOR`, and compose with grep, awk, pipes, and shell
completion.

`nt` is not an agent framework, RAG system, vector database, daemon, server,
browser/runtime orchestrator, microVM orchestrator, workflow engine, Codex
launcher, or Hermes replacement.

## Rules

- The binary name is `nt`.
- Core workflows are flagless.
- Prefer positional arguments, stdin, stdout, and `$EDITOR`.
- Note ids use `NTYYYYMMDDTHHmmss`.
- Note filenames use `<id>.md`.
- The notes directory is flat and contains only atomic `.md` note files.
- Notes are plain CommonMark Markdown.
- Do not store metadata in Markdown front matter.
- Do not add wiki-link syntax or nt-specific note-body markup.
- Store metadata in `$HOME/.nt/index.json`.
- Write notes and indexes atomically with temp-file-and-rename.
- Do not add a database, daemon, embeddings, vector store, RAG, or hidden
  retrieval layer.
- Do not add hidden agent-only behavior.
- Do not add built-in agent launchers such as `nt agent` or `nt discuss`.
- Do not create `AGENTS.md`, skill files, or agent workspaces during `nt init`.
- Provide command and note id completion.
- Use `clap` and `clap_complete` for CLI behavior.
- Use `serde` and `serde_json` for JSON.
- Use `thiserror` for application errors.

## Commands

Implemented command surface (see `docs/cli-reference.md`):

- `nt init <notes-dir>`
- `nt add [metadata...]`
- `nt rebuild`
- `nt list`
- `nt list all [filter...]`
- `nt list <field>[,<field>...] [filter...]`
- `nt list ids`
- `nt list tags`
- `nt list collections`
- `nt list links [filter...]`
- `nt find <expr...>`
- `nt show <id>`
- `nt open <id>`
- `nt rm <id...>`
- `nt update <id> <field> <value>`
- `nt agenda [today|week|overdue|waiting|undated]`
- `nt export <path> [id...]`
- `nt config show`
- `nt config vault`
- `nt config vault <vault-name>`
- `nt completion <shell>`
- `nt help`
- `nt help <command>`

Avoid adding broader commands such as `search`, `grep`, `graph`,
`browse`, `agent`, `discuss`, workflow orchestration, or runtime management
until real usage proves they are necessary.

## Query Syntax

The canonical CLI command and query syntax lives in `docs/cli-reference.md`.
Agents should follow that file when constructing commands.

`nt find` uses trailing positional query expressions:

```sh
nt find qemu firecracker
nt find tag:decision qemu
nt find since:2026-05-01 before:2026-06-01 tag:decision collection:projects/nt
nt find kind:meeting
nt find status:open
nt find collection:meetings
nt find link:NT20260528T143012
nt find source:firecracker
nt find body:'microvm jailer'
nt find not:tag:draft qemu
```

Rules:

- Each positional argument is one query expression.
- All expressions are combined with `AND`.
- Expression order does not matter.
- Search is case-insensitive.
- Bare words match searchable metadata or note bodies.
- `#tag` is shorthand for `tag:<tag>`.
- Quoted values rely on normal shell quoting, not separate nt query syntax.
- Unknown fields are errors, not bare-word searches.
- Avoid full boolean syntax, parentheses, scoring, fuzzy search, and regex by
  default.

Initial query fields:

- `id:<id>`
- `tag:<tag>`
- `title:<term>`
- `day:<YYYY-MM-DD>`
- `since:<YYYY-MM-DD>`
- `before:<YYYY-MM-DD>`
- `kind:<kind>`
- `status:<status>`
- `priority:<priority>`
- `scheduled:<YYYY-MM-DD>`
- `due:<YYYY-MM-DD>`
- `closed:<YYYY-MM-DD>`
- `collection:<name>`
- `link:<id>`
- `source:<term>`
- `body:<term>`
- `not:<expr>`

## Storage

- Store note bodies under the configured notes directory.
- Keep that directory flat: only `NTYYYYMMDDTHHmmss.md` files.
- The id is the filename stem.
- Store metadata under `$HOME/.nt/index.json`.
- Use a metadata map keyed by note id for direct lookup.
- Keep derived maps for fast lookup.
- Do not store note bodies in the index.

Primary note metadata should stay small:

- `id`
- `path`
- `created`
- `updated`
- `title`
- `kind`
- `status`
- `priority`
- `scheduled`
- `due`
- `closed`
- `tags`
- `collections`
- `links`
- `sources`

Derived maps may include:

- `recent`
- `kinds`
- `statuses`
- `tags`
- `collections`
- `days`
- `backlinks`
- `terms`

Derived maps must be rebuildable from primary metadata and, where useful, from
CommonMark note bodies.

## Metadata Model

Use distinct fields instead of overloading tags:

- `kind`: the structural form of a note, such as `note`, `todo`, `meeting`,
  `decision`, `source`, `research`, or `project`.
- `status`: agenda state, such as `open`, `waiting`, `done`, or `dropped`.
- `priority`: optional urgency ordered `S`, `A`, `B`, `C`, `D`.
- `scheduled`: optional calendar date when a todo should appear.
- `due`: optional calendar date for a todo, formatted as `YYYY-MM-DD`.
- `closed`: system-managed UTC timestamp for the terminal status transition.
- `collection`: where a note belongs, such as `todos`, `meetings`,
  `projects/nt`, or `research/qemu`.
- `tag`: sparse topics or entities.
- `link`: exact note-to-note relationships stored in JSON metadata.
- `source`: external source references.

Tags should stay sparse. Agents should run `nt list tags` before choosing tags,
prefer existing tags, use one to three tags by default, and create a new tag
only when the concept is likely to recur.

Collections are workspace-like groups. Collection names should be lowercase and
may use `/` as a naming convention, not nested file storage.

Note-to-note links live in JSON metadata. Do not require special Markdown link
syntax for note links.

## Agent Flow

Agents should retrieve notes through cheap, visible operations:

- Use `nt list id` for completion and direct id lists (`nt list ids` remains a
  compatibility alias).
- Use `nt list` for the `id`, `title`, `kind`, `status`, `due`, and `tag`
  summary in active-recent order; use `nt list all` for every indexed field.
- Use projections such as `nt list id,title,status status:open` for stable,
  tab-separated metadata rows and exact structured filtering.
- Use `nt list tags` and `nt list collections` before choosing metadata.
- Use `nt find <expr...>` for indexed/body search.
- Use `nt show <id>` for exact retrieval.
- Use `nt list links`, `nt list links from:<id>`, and `nt list links to:<id>` for
  explicit note relationships.
- Compose command output with normal Unix tools when helpful.

When answering from notes, cite supporting note ids.

Agent-driven writes require approval before mutation:

- New notes: produce a CommonMark draft and ask before saving with `nt add`.
- Note edits: produce a proposed replacement or patch, then open `$EDITOR`
  before saving.
- Metadata updates: show planned `nt update <id> <field> <value>` commands
  before running them.

Rejection must leave notes and metadata unchanged.

Agent skill examples belong in documentation, not runtime initialization.
Repository-local contributor skills under `.agents/skills/` must not be copied
into user vaults by `nt init`.

## Terminal UX

`nt` output should be minimal, fast, predictable, and grep-friendly:

- Successful mutations print one short line, such as `saved <id>`.
- Lists use aligned columns: id, date, tags, title.
- `show` prints note identity and metadata before the CommonMark body.
- Prefer lowercase verbs in status output.
- Keep ids visually dominant.
- Keep paths relative when possible.
- Avoid decorative boxes, banners, spinners, and progress bars.
- Use ANSI color only when stdout is a TTY.
- Disable color when stdout is piped, `NO_COLOR` is set, or `TERM=dumb`.
- Machine-facing `list` submodes and `find` must stay stable and
  one-record-per-line.

Suggested TTY colors:

- ids: bright cyan
- dates and paths: dim
- tags: green
- errors: red

## Coding Style

- Keep modules small.
- Prefer explicit control flow.
- Prefer standard library APIs.
- Avoid clever abstractions.
- Avoid dependencies unless they clearly simplify stable core behavior.
- Keep terminal output readable.
- Keep error messages actionable.
- Do not hand-roll JSON parsing.

## Testing

- Run `cargo fmt` before finishing Rust changes.
- Run `cargo test` when behavior changes.
- Run `cargo run -- help` for a basic command smoke test.
- Add focused tests for command routing, note ids, atomic writes, index updates,
  completion, parsing, query syntax, metadata mutation commands, and storage.

## Commits

Use concise conventional commit prefixes:

- `fix: ...`
- `refactor: ...`
- `chore: ...`
- `docs: ...`
- `test: ...`

Keep each commit focused on one kind of change. Do not mix documentation-only
changes, behavior changes, refactors, chores, and tests unless they are tightly
coupled.
