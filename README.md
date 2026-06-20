# nt

`nt` is a small CLI-native note organizer: canonical CommonMark notes in an
active vault, visible JSON metadata, deterministic search, and shell-friendly
commands.

It is built for humans and agents that already know how to use Unix tools. It
reads stdin, writes stdout, opens `$EDITOR`, exposes stable one-record-per-line
commands, and does not keep a hidden memory layer.

See [docs/usage.md](docs/usage.md) for a compact guide,
[docs/cli-syntax-spec.md](docs/cli-syntax-spec.md) for the command/query
contract, [docs/shell-workflows.md](docs/shell-workflows.md) for shell-first
workflows, [docs/nt-core-v1.md](docs/nt-core-v1.md) for the readiness audit,
[docs/design.md](docs/design.md) for boundaries, and
[docs/examples/agent-skills.md](docs/examples/agent-skills.md) for optional
agent skill examples. See [CHANGELOG.md](CHANGELOG.md) for release notes and
[docs/release-checklist.md](docs/release-checklist.md) for the manual release
checklist.

## Status

`nt` 0.1.0 is usable as the initial stable core.

The documentation now defines the next command surface: consolidated `list`
submodes, `update`, and a read-only `agenda`. The released binary still exposes
the legacy metadata commands until the staged work in
[docs/command-surface-implementation-plan.md](docs/command-surface-implementation-plan.md)
is complete.

The core model is intentionally small:

- canonical CommonMark notes in an active vault
- visible JSON index at `$HOME/.nt/index.json`
- rebuildable metadata and body term indexes
- indexed `nt find` candidate narrowing
- deterministic active-recent output
- shell-first workflows

Future work should be fixes, polish, and features layered on this core, not a
redesign of the storage/search model.

### Target command surface

- `init`, `add`, `list`, `find`, `show`, `open`, and `rm`
- consolidated metadata updates through `nt update`
- a read-only todo agenda with scheduling, deadlines, five priorities, and
  completion timestamps
- `rebuild`
- active vault config
- completion generation
- shell-first composition
- agent-compatible CLI behavior

### Not in core

There is no TUI, RAG, embeddings, ranking, daemon, server, workflow engine, or
hidden agent-only interface. A TUI is intentionally deferred and is not part of
the current core.

## Install

From source:

```sh
cargo install --path .
```

Requirements:

- Rust toolchain
- Unix-like shell recommended
- `$EDITOR` for interactive note capture/editing

Homebrew, crates.io, and binary releases are not available yet.

## Quick Start

```sh
nt init notes
printf '%s\n' '# First Note' '' 'body text' | nt add tag:example
nt find example
```

`nt add` prints a note id like `NT20260616T101500`. Use that id with:

```sh
nt show <id>
nt open <id>
```

Run `nt rebuild` after out-of-band file edits or deletes.

## Core Model

Markdown files are canonical. The active vault is flat and contains only
`NTYYYYMMDDTHHmmss.md` note files.

Metadata lives in `$HOME/.nt/index.json`, not Markdown front matter. The index
stores vault config, note metadata, rebuildable derived maps, and body term
indexes. It does not store note bodies.

`nt rebuild` reconstructs the active vault visible index from Markdown note
files and visible JSON metadata: it preserves primary metadata, preserves
existing sources and merges URLs currently found in Markdown body, removes stale
active-vault entries, cleans links to deleted notes, and refreshes the body term
index.

## Commands

```sh
nt init <notes-dir>
nt add [metadata...]
nt rebuild
nt list
nt list ids
nt list tags [tag]
nt list collections [collection]
nt list links <id> [from|to]
nt find <expr...>
nt show <id>
nt open <id>
nt rm <id>
nt update <id> <field> <value>
nt agenda [today|week|overdue|waiting|undated]
nt export <path> [id...]
nt config show
nt config vault
nt config vault <vault-name>
nt completion <shell>
nt help
nt help <command>
```

## Search

`nt find` takes positional query expressions. All expressions are combined with
`AND`; order does not matter; search is case-insensitive. It uses visible
metadata and the body term index for candidate narrowing where available, then
prints verified matches in deterministic active-recent order. Missing body index
entries may fall back to Markdown body reads. Indexed body entries are trusted
until `nt rebuild`. Quoted multiword `body:` values match all indexed terms, not
an exact phrase. There is no ranking, fuzzy search, or semantic search.

```sh
nt find qemu firecracker
nt find tag:decision collection:projects/nt
nt find since:2026-05-01 before:2026-06-01 not:tag:draft
nt find body:'microvm jailer'
```

Common expressions:

```text
qemu                  metadata or indexed body contains qemu
#vm                   shorthand for tag:vm
tag:decision          exact tag
title:storage         title contains storage
kind:meeting          exact kind
status:open           exact status
priority:S            exact priority
scheduled:2026-06-25  exact scheduled date
due:2026-06-30        exact due date
closed:2026-06-30     closed during the UTC calendar day
collection:projects/nt
day:2026-05-28
since:2026-05-01
before:2026-06-01
link:NT20260605T101500
source:firecracker
body:'microvm jailer'  body contains terms microvm AND jailer
not:tag:draft
```

Unknown fields are errors so typos do not silently become broad text searches.

## Shell-first Workflows

`nt` keeps the core loop to `nt find`, `nt show`, and `nt open`. Paging, fuzzy
selection, previews, and batching come from shell tools such as `less`, `fzf`,
`awk`, and `xargs`.

See [docs/shell-workflows.md](docs/shell-workflows.md) for optional recipes.

## Agent Use

`nt` has no built-in agent command. Any agent that can run shell commands can
use the same visible workflow:

```sh
nt help
nt list
nt list tags
nt list collections
nt find collection:projects/nt status:open
nt show NT20260528T143012
```

When an agent writes notes, it should draft CommonMark, ask before mutation when
appropriate, then save through `nt add` or open through `nt open`. Optional
skill examples live in [docs/examples/agent-skills.md](docs/examples/agent-skills.md)
so users can adapt them to Codex, Claude Code, Cursor, or any other agent system
without `nt` owning that runtime.

## Development / Release

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets
cargo run -- help
```
