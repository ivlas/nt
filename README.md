# nt

`nt` is a small note organizer and CLI research workspace for humans and
agents.

Its primary design target is agent use: plain-text knowledge, visible commands,
deterministic retrieval, editable Markdown, and no hidden memory layer. Humans
use the same Unix-like interface: read stdin, write stdout, use `$EDITOR`, store
plain files, and compose with shell tools.

In spirit, `nt` is org-mode for agents, but smaller and CLI-native. It is a
knowledge substrate, note organizer, and "from CLI" research assistant layer. It
is not an agent framework, RAG system, vector database, daemon, server, browser
runtime, microVM orchestrator, or Hermes replacement.

Notes are atomic Markdown files. Metadata is a visible JSON index under
`$HOME/.nt`. There is no database, daemon, embeddings, vector store, hidden
retrieval, or RAG.

See [docs/usage.md](docs/usage.md) for a compact usage guide and
[docs/design.md](docs/design.md) for the project boundaries.

## Goals

- Capture notes quickly.
- Make agent recall explicit, inspectable, and reproducible.
- Retrieve a note by id in one direct path lookup.
- Keep notes readable and editable without `nt`.
- Keep metadata simple, visible, and rebuildable where possible.
- Make agent use predictable with plain, grep-friendly output.
- Stay flagless for core workflows.
- Provide shell completion for commands and note ids.

## Design Loop

The core loop is:

```text
capture -> organize -> retrieve -> inspect -> revise -> rebuild
```

`nt` keeps that loop visible:

```sh
nt add
nt list
nt find <query>
nt show <id>
nt edit <id>
nt tags
nt rebuild
nt agent <prompt...>
```

Agents should use the same commands humans use. For example, an agent can find
candidate notes with `nt find qemu`, inspect exact Markdown with
`nt show NT20260528T143012`, revise a note with `nt edit <id>`, and rebuild the
index with `nt rebuild` if metadata gets stale.

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
nt config show
nt config agent-output <hidden|format|full>
nt agent <prompt...>
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
nt agent note this decision about metadata outside markdown
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

For large vaults, commands should avoid pretty output internally and expose
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

## Codex Agent

`nt agent <prompt...>` is a thin Codex launcher. It loads visible nt skills from
the active vault, builds a prompt, and runs `codex exec`. `nt` does not
implement natural-language retrieval itself; the agent is expected to call
explicit commands such as `nt find`, `nt list`, and `nt show`.

Default skills are created by `nt init` and are editable Markdown files. Use
`nt config show` to see the active vault, agent workspace, and available
skills.

The default skills are:

- `nt-note`: capture compact research, context, and decisions with `nt add`.
- `nt-recall`: retrieve with visible `nt list`, `nt find`, and `nt show`
  commands, then cite note ids.
- `nt-maintain`: inspect and repair the vault/index with `nt ids`,
  `nt tags`, and `nt rebuild`.
- `nt-skill-builder`: help create or refine custom nt skills for the vault.

Agent output is configured in `$HOME/.nt/config.json`:

```sh
nt config agent-output hidden
nt config agent-output format
nt config agent-output full
nt config show
```

`format` is the default. It hides Codex session metadata and prints the extracted
assistant answer. `full` streams the complete Codex output. `hidden` prints only
status lines.

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
