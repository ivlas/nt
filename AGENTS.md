# AGENTS.md

## Project

`nt` is a small note-taking CLI for humans and agents.

Notes are atomic Markdown files. Metadata is stored as visible JSON under
`$HOME/.nt`. Commands should feel like Unix tools: read stdin, write stdout, use
`$EDITOR`, and compose with grep, awk, pipes, and shell completion.

## Rules

- The binary name is `nt`.
- The tool is flagless for core workflows.
- Note ids use `NTYYYYMMDDTHHmmss`.
- Note filenames use `<id>.md`.
- The notes directory contains only atomic `.md` note files.
- Do not store metadata in Markdown front matter.
- Store metadata in `$HOME/.nt/index.json`.
- Write notes and indexes atomically with temp-file-and-rename.
- Do not add a database, daemon, embeddings, vector store, or RAG.
- Do not add hidden agent-only behavior.
- Provide command and note id completion.
- Use `clap` and `clap_complete` for CLI behavior.
- Use `thiserror` for application errors.

## Commands

Start with this compact surface:

- `nt init <notes-dir>`
- `nt add`
- `nt list`
- `nt show <id>`
- `nt edit <id>`
- `nt find <query>`
- `nt ids`
- `nt tags`
- `nt rebuild`
- `nt rm <id>`
- `nt completion <shell>`
- `nt skill install`
- `nt skill list`
- `nt skill show <name>`
- `nt config show`
- `nt config agent-output <hidden|format|full>`
- `nt agent <prompt...>`

Prefer positional arguments, stdin, stdout, and `$EDITOR` over flags.

## Storage

- Store note bodies under the configured notes directory.
- Keep that directory flat: only `NTYYYYMMDDTHHmmss.md` files.
- The id is the filename stem.
- Store metadata under `$HOME/.nt/index.json`.
- Use a metadata map keyed by note id for direct lookup.
- Keep simple secondary indexes for common filters, such as recent ids, tags,
  and days.
- Support `nt rebuild` to recover derived metadata from the notes directory.
- Agents should use `nt` commands when they exist.
- Direct file edits are acceptable only when no command exists yet.

## Metadata

Metadata should stay small and useful for filtering:

- `id`
- `path`
- `created`
- `updated`
- `title`
- `tags`

The index may include derived maps such as `recent`, `tags`, and `days` when
they make filtering faster. Derived maps must be rebuildable from the primary
note metadata.

Do not store note bodies in the index. For large notebooks, `nt find <query>`
should check metadata first and stream Markdown files only when body search is
needed.

## Agent Flow

Agents should retrieve notes through cheap, visible operations:

- Use `nt ids` for completion and direct id lists.
- Use `nt list` for recent note summaries.
- Use `nt find <query>` for simple indexed/body search.
- Use `nt show <id>` for exact retrieval.
- Compose command output with normal Unix tools when helpful.

No command should require hidden retrieval, embeddings, or external services.

`nt agent <prompt...>` is a thin Codex launcher. It must rely on nt skills from
`$HOME/.nt/skills` and shell out to `codex exec`; it must not implement natural
language retrieval itself.

Use `nt skill install` to create the default self-referential nt skills:

- `nt-note`
- `nt-recall`
- `nt-maintain`

These skills describe how an agent should navigate `nt` commands. They are
editable Markdown files and should stay agent-agnostic where possible.

Agent output is controlled by `$HOME/.nt/config.json`:

- `hidden`: print status only.
- `format`: print the extracted Codex answer.
- `full`: print the full Codex output.

## Terminal UX

`nt` output should be minimal, fast, and predictable:

- Successful mutations print one short line, such as `saved <id>`.
- Lists use aligned columns: id, date, tags, title.
- `show` prints note identity, path, then the Markdown body.
- Prefer lowercase verbs in status output.
- Keep ids visually dominant.
- Keep paths relative when possible.
- Avoid decorative boxes, banners, spinners, and progress bars.
- Use ANSI color only when stdout is a TTY.
- Disable color when stdout is piped, `NO_COLOR` is set, or `TERM=dumb`.
- Machine-facing commands such as `ids`, `find`, and `tags` must stay stable
  and one-record-per-line.

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
  completion, parsing, and storage.

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
