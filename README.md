# nt

`nt` is a small note-taking CLI for humans and agents.

It uses a natural Unix-like flow: commands read plain files, write plain files,
and print plain text. Notes are Markdown. There is no database, daemon, hidden
index, embeddings, vector store, or expensive retrieval layer.

## Goals

- Capture notes quickly.
- Keep notes readable and editable without `nt`.
- Make agent use boring and inspectable.
- Stay flagless for core workflows.
- Use `clap` for commands and `thiserror` for errors.

## Commands

```sh
nt init
nt add
nt list
nt show <id>
nt edit <id>
nt find <query>
nt rm <id>
```

Core commands should use positional arguments, stdin, stdout, and `$EDITOR`
instead of flags.

Examples:

```sh
nt add
echo "Remember the storage shape" | nt add
nt list
nt show 20260528-143012
nt find storage
nt edit 20260528-143012
nt rm 20260528-143012
```

## Storage

Default storage is a visible local directory:

```text
notes/
  2026/
    05/
      20260528-143012.md
```

Each note is Markdown with optional plain front matter:

```markdown
---
id: 20260528-143012
created: 2026-05-28T14:30:12+02:00
---

# Storage shape

Keep the note format simple.
```

The file tree is the source of truth. If `nt` breaks, the notes are still just
files.

## Design

- Keep modules small.
- Prefer explicit control flow.
- Prefer standard library APIs.
- Keep output readable.
- Keep errors actionable.
- Avoid dependencies unless they simplify stable core behavior.

Suggested Rust shape:

- CLI parsing in `main.rs`.
- Command handlers in a command module.
- Note parsing and formatting in a note module.
- File layout and reads/writes in a storage module.
- Application errors with `thiserror`.

## Development

```sh
cargo fmt
cargo test
cargo run -- help
```
