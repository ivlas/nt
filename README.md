# nt

`nt` is a small note-taking CLI for humans and agents.

It uses a Unix-like flow: read stdin, write stdout, use `$EDITOR`, store plain
files, and keep retrieval cheap. Notes are atomic Markdown files. Metadata is a
visible JSON index under `$HOME/.nt`.

There is no database, daemon, embeddings, vector store, or RAG.

## Goals

- Capture notes quickly.
- Retrieve a note by id in one direct path lookup.
- Keep notes readable and editable without `nt`.
- Keep metadata simple, visible, and rebuildable where possible.
- Make agent use predictable with plain text output.
- Stay flagless for core workflows.
- Provide shell completion for commands and note ids.

## Commands

```sh
nt init <notes-dir>
nt add
nt list
nt show <id>
nt edit <id>
nt find <query>
nt ids
nt tags
nt rebuild
nt rm <id>
nt completion <shell>
```

Core commands use positional arguments, stdin, stdout, and `$EDITOR` instead of
flags.

Examples:

```sh
nt init notes
echo "Remember the storage shape" | nt add
nt list
nt ids
nt show NT20260528T143012
nt find storage
nt edit NT20260528T143012
nt rebuild
nt completion zsh
```

## Note Files

The notes directory contains only note files:

```text
notes/
  NT20260528T143012.md
  NT20260528T150501.md
```

The filename stem is the note id:

```text
NTYYYYMMDDTHHmmss
```

A note file contains only Markdown content, with no front matter:

```markdown
# Storage shape

Keep the note format simple.
```

Writes should be atomic: create a temporary file in the same directory, write
the complete note, sync it, then rename it to `NTYYYYMMDDTHHmmss.md`.

## Metadata

Metadata lives in `$HOME/.nt/index.json`, not in Markdown headers.

Suggested shape:

```json
{
  "version": 1,
  "active_notes_dir": "/Users/you/project/notes",
  "notebooks": {
    "/Users/you/project/notes": {
      "created": "2026-05-28T14:30:12+02:00"
    }
  },
  "notes": {
    "NT20260528T143012": {
      "id": "NT20260528T143012",
      "path": "/Users/you/project/notes/NT20260528T143012.md",
      "created": "2026-05-28T14:30:12+02:00",
      "updated": "2026-05-28T14:30:12+02:00",
      "title": "Storage shape",
      "tags": ["design"]
    }
  },
  "recent": ["NT20260528T143012"],
  "tags": {
    "design": ["NT20260528T143012"]
  },
  "days": {
    "2026-05-28": ["NT20260528T143012"]
  }
}
```

The `notes` map gives direct lookup by id. The `tags` and `days` maps are small
secondary indexes for fast filtering. The index should be written atomically the
same way as notes.

`nt rebuild` should scan the notes directory and recreate metadata that can be
derived from filenames and Markdown content. User-authored metadata that cannot
be derived, such as tags, belongs in `$HOME/.nt/index.json`.

## Scale

The index is allowed to duplicate cheap metadata so common operations stay fast:

- `nt show <id>` reads `notes[id].path` and opens one file.
- `nt ids` reads keys or the `recent` list from the index.
- `nt list` reads metadata only, not note bodies.
- `nt tags` reads the tag map.
- `nt find <query>` checks metadata first, then streams note bodies when needed.

For 10k to 100k notes, avoid loading note bodies into the index. Keep the index
small enough to rewrite atomically, and keep full-text search as a streaming file
operation unless a plain, rebuildable on-disk index becomes necessary.

For large notebooks, commands should avoid pretty output internally and expose
agent-friendly streams:

```sh
nt ids
nt list
nt find rust
nt tags
```

These commands should print stable, grep-friendly lines so agents can compose
them with normal Unix tools.

## Terminal Style

`nt` should feel fast, quiet, and sharp.

Use compact one-line output for successful commands:

```text
saved NT20260528T143012
removed NT20260528T143012
```

Use aligned list output for humans:

```text
NT20260528T143012  2026-05-28  design       Storage shape
NT20260528T150501  2026-05-28  rust,cli     Completion behavior
```

Use direct note output for `show`:

```text
NT20260528T143012  Storage shape
path notes/NT20260528T143012.md

# Storage shape

Keep the note format simple.
```

For command output:

- Prefer lowercase verbs: `saved`, `removed`, `indexed`, `missing`.
- Keep ids visually dominant.
- Keep dates short in lists.
- Keep paths relative when possible.
- Avoid decorative boxes, banners, spinners, and progress bars.
- Use ANSI color only when stdout is a TTY.
- Disable color when stdout is piped, `NO_COLOR` is set, or `TERM=dumb`.

Suggested TTY color style:

- ids: bright cyan
- titles: default foreground
- dates and paths: dim
- tags: green
- errors: red

Machine-facing output should stay plain and stable:

```sh
nt ids
nt find rust
nt tags
```

These commands should avoid ANSI styling and print one record per line.

## Completion

Use `clap_complete` for shell command completion:

```sh
nt completion zsh
nt completion bash
nt completion fish
```

Note id completion should be dynamic and backed by the JSON index. The generated
completion script can call `nt ids` to complete note ids without a daemon.

## Design

- Keep modules small.
- Prefer explicit control flow.
- Prefer standard library APIs.
- Keep output readable.
- Keep errors actionable.
- Use `clap` and `clap_complete` for CLI behavior.
- Use `thiserror` for application errors.
- Add JSON support deliberately; hand-written JSON parsing is not worth it.

Suggested Rust shape:

- CLI parsing in `main.rs`.
- Command handlers in a command module.
- Atomic file writes in a filesystem module.
- Note id and note body handling in a note module.
- Metadata index reads/writes in an index module.
- Completion generation in a completion module.
- Application errors with `thiserror`.

## Development

```sh
cargo fmt
cargo test
cargo run -- help
```
