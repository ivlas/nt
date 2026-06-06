# AGENTS.md

## Project

`nt` is a small CLI-native note organizer and research workspace for humans and
agents. The primary design target is coding agents: visible commands,
deterministic retrieval, editable CommonMark notes, and no hidden memory layer.

Humans use the same Unix-like interface. Commands should read stdin, write
stdout, use `$EDITOR`, and compose with grep, awk, pipes, and shell completion.

`nt` is not an agent framework, RAG system, vector database, daemon, server,
browser/runtime orchestrator, microVM orchestrator, workflow engine, or Hermes
replacement.

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
- Provide command and note id completion.
- Use `clap` and `clap_complete` for CLI behavior.
- Use `serde` and `serde_json` for JSON.
- Use `toml` for `$HOME/.nt/config.toml`.
- Use `thiserror` for application errors.

## Commands

Recommended command surface:

- `nt init <notes-dir>`
- `nt add`
- `nt list`
- `nt find <expr...>`
- `nt show <id>`
- `nt edit <id>`
- `nt discuss <id>`
- `nt discuss <id> <prompt...>`
- `nt rm <id>`
- `nt rebuild`
- `nt ids`
- `nt tags`
- `nt collections`
- `nt collection <name>`
- `nt collect <id> <collection>`
- `nt uncollect <id> <collection>`
- `nt kind <id> <kind>`
- `nt status`
- `nt status <id> <status>`
- `nt link <from-id> <to-id>`
- `nt unlink <from-id> <to-id>`
- `nt links <id>`
- `nt backlinks <id>`
- `nt agent <prompt...>`
- `nt config show`
- `nt config agent-output <hidden|format|full>`
- `nt completion <shell>`

Avoid adding broader commands such as `search`, `grep`, `graph`, `open`,
`browse`, workflow orchestration, or runtime management until real usage proves
they are necessary.

## Query Syntax

The canonical CLI command and query syntax lives in `docs/cli-syntax-spec.md`.
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
nt find backlink:NT20260528T143012
nt find ref:firecracker
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
- `collection:<name>`
- `link:<id>`
- `backlink:<id>`
- `ref:<term>`
- `body:<term>`
- `not:<expr>`

Use `docs/cli-syntax-spec.md` as the source of truth if this summary drifts.

## Storage

- Store note bodies under the configured notes directory.
- Keep that directory flat: only `NTYYYYMMDDTHHmmss.md` files.
- The id is the filename stem.
- Store metadata under `$HOME/.nt/index.json`.
- Use a metadata map keyed by note id for direct lookup.
- Keep derived maps for fast lookup.
- Support `nt rebuild` to recover rebuildable metadata.
- Do not store note bodies in the index.

Primary note metadata should stay small:

- `id`
- `path`
- `created`
- `updated`
- `title`
- `kind`
- `status`
- `tags`
- `collections`
- `links`
- `refs`

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

Metadata fields that cannot be derived from CommonMark must be updated through
explicit commands such as `nt collect`, `nt kind`, `nt status`, and `nt link`.
Do not edit `$HOME/.nt/index.json` directly unless no command exists and the
repair cannot be done with `nt rebuild`.

## Metadata Model

Use distinct fields instead of overloading tags:

- `kind`: the structural form of a note, such as `note`, `todo`, `meeting`,
  `decision`, `source`, `research`, or `project`.
- `status`: agenda state, such as `open`, `waiting`, `done`, or `dropped`.
- `collection`: where a note belongs, such as `todos`, `meetings`,
  `projects/nt`, or `research/qemu`.
- `tag`: sparse topics or entities.
- `link`: exact note-to-note relationships stored in JSON metadata.
- `ref`: external source references.

Tags should stay sparse. Agents should run `nt tags` before choosing tags,
prefer existing tags, use one to three tags by default, and create a new tag
only when the concept is likely to recur.

Collections are workspace-like groups. Collection names should be lowercase and
may use `/` as a naming convention, not nested file storage.

Note-to-note links live in JSON metadata. Do not require special Markdown link
syntax for note links.

## Agent Flow

Agents should retrieve notes through cheap, visible operations:

- Use `nt ids` for completion and direct id lists.
- Use `nt list` for recent note summaries.
- Use `nt tags` and `nt collections` before choosing metadata.
- Use `nt find <expr...>` for indexed/body search.
- Use `nt show <id>` for exact retrieval.
- Use `nt links <id>` and `nt backlinks <id>` for explicit note relationships.
- Compose command output with normal Unix tools when helpful.

When answering from notes, cite supporting note ids.

No command should require hidden retrieval, embeddings, external services, or
private agent memory.

`nt agent <prompt...>` is a thin Codex launcher. It must rely on nt skills from
the active workspace and shell out to `codex exec`; it must not implement natural
language retrieval itself.

`nt discuss <id>` is the interactive counterpart. It should open Codex with
`nt show <id>` output and visible metadata as context so the user can continue a
discussion from a specific note.

Agent-driven writes require approval before mutation:

- New notes: produce a CommonMark draft and ask before saving with `nt add`.
- Note edits: produce a proposed replacement or patch, then open `$EDITOR`
  before saving.
- Metadata updates: show planned commands such as `nt collect`, `nt link`,
  `nt kind`, or `nt status` before running them.

Rejection must leave notes and metadata unchanged.

Default nt skills should be created automatically by `nt init`; there should be
no separate skill install/list/show command group. `nt config show` should show
the active notes directory, agent workspace, and available skill names/paths.

Default self-referential nt skills:

- `nt-note`
- `nt-recall`
- `nt-maintain`
- `nt-skill-builder`

These skills describe how an agent should navigate `nt` commands. They are
editable Markdown files and should stay agent-agnostic where possible.

`nt-skill-builder` helps the user create or refine custom nt skills for the
current workspace. Custom skills are plain editable Markdown files in the active
nt skills directory.

Agent output is controlled by `$HOME/.nt/config.toml`:

- `hidden`: print status only.
- `format`: print the extracted Codex answer.
- `full`: print the full Codex output.

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
- Machine-facing commands such as `ids`, `find`, `tags`, `collections`,
  `links`, and `backlinks` must stay stable and one-record-per-line.

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
