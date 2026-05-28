# AGENTS.md

## Project

`nt` is a small note-taking CLI for humans and agents.

Notes are Markdown files. Commands should feel like normal Unix tools: read
stdin when useful, write stdout, use `$EDITOR`, and keep state visible on disk.

## Rules

- The binary name is `nt`.
- The tool is flagless for core workflows.
- Notes are plain Markdown files.
- Plain files are the source of truth.
- Durable state must be inspectable without `nt`.
- Do not add a database, daemon, hidden index, embeddings, vector store, or RAG.
- Do not add hidden agent-only behavior.
- Use `clap` for command parsing.
- Use `thiserror` for application errors.

## Commands

Start with this compact surface:

- `nt init`
- `nt add`
- `nt list`
- `nt show <id>`
- `nt edit <id>`
- `nt find <query>`
- `nt rm <id>`

Prefer positional arguments, stdin, stdout, and `$EDITOR` over flags.

## Storage

- Store notes under a visible `notes/` directory by default.
- Use stable ids that can be derived from filenames.
- Keep metadata visible in the note file when metadata is needed.
- Agents should use `nt` commands when they exist.
- Direct file edits are acceptable only when no command exists yet.

## Coding Style

- Keep modules small.
- Prefer explicit control flow.
- Prefer standard library APIs.
- Avoid clever abstractions.
- Avoid dependencies unless they clearly simplify stable core behavior.
- Keep terminal output readable.
- Keep error messages actionable.

## Testing

- Run `cargo fmt` before finishing Rust changes.
- Run `cargo test` when behavior changes.
- Run `cargo run -- help` for a basic command smoke test.
- Add focused tests for command routing, note ids, parsing, and storage.
