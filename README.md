# nt

`nt` is a small CLI-native note organizer: canonical CommonMark notes in a flat
vault, visible JSON metadata, deterministic search, and shell-friendly commands.
Humans and agents share the same Unix interface — stdin, stdout, `$EDITOR`,
one-record-per-line output, no hidden memory layer.

## Install

```sh
cargo install --path .
```

Requires a Rust toolchain and `$EDITOR` for interactive capture/editing.

## Quick Start

```sh
nt init notes
printf '%s\n' '# First Note' '' 'body text' | nt add tag:example
nt find example          # prints NT20260616T101500-style ids
nt show <id>
nt open <id>             # edit in $EDITOR
nt list                  # id title kind status due tag
nt agenda                # open/waiting todos
nt rebuild               # after out-of-band file edits or deletes
```

## Core Model

- Note bodies are plain CommonMark in a flat vault of `NTYYYYMMDDTHHmmss.md`
  files. The first non-empty line is the required `# Title`.
- Metadata lives in `$HOME/.nt/index.json`, not Markdown front matter. The
  index stores vault config, primary metadata, and rebuildable derived maps and
  text indexes — never note bodies.
- `nt find` is deterministic `AND`-combined, case-insensitive filtering with
  exact metadata fields (`tag:`, `kind:`, `status:`, `collection:`, `due:`,
  `not:`, …) and bare/body terms. No ranking, fuzzy, or semantic search.
- Mutations print one short line (`saved <id>`, `updated <id> …`). Reads are
  one record per line. ANSI color is TTY-only.

## Documentation

- [docs/usage.md](docs/usage.md) — task-oriented workflows and shell recipes
- [docs/cli-reference.md](docs/cli-reference.md) — complete command, query,
  value, and output contract
- [docs/design.md](docs/design.md) — architecture and decisions

## Development

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets
cargo run -- help
```
