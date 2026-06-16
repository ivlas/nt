# nt

`nt` is a Markdown-first, Git-friendly personal knowledge index: canonical
CommonMark notes, visible JSON metadata, deterministic search, and
shell-friendly commands. Its core goal is `time-to-knowledge`: the shortest path
from vague memory to an exact note id and the note content behind it.

It is intentionally useful to both humans and coding agents because it behaves
like a normal Unix tool. It reads stdin, writes stdout, opens `$EDITOR`, exposes
stable one-record-per-line commands, and does not keep a hidden memory layer.

Markdown files are canonical. `$HOME/.nt/index.json` is the visible index: it
stores vault config, note metadata, rebuildable derived maps, and body term
indexes. It does not replace the Markdown note body.

`nt` is not an app framework, agent runtime, RAG system, vector database,
daemon, server, browser/runtime orchestrator, workflow engine, or launcher for a
specific agent. Agents can still use it directly through zsh/bash by reading
`nt help`, running `nt find`, and inspecting exact notes with `nt show`.

See [docs/usage.md](docs/usage.md) for a compact guide,
[docs/cli-syntax-spec.md](docs/cli-syntax-spec.md) for the command/query
contract, [docs/shell-workflows.md](docs/shell-workflows.md) for shell-first
human workflows, [docs/design.md](docs/design.md) for boundaries, and
[docs/examples/agent-skills.md](docs/examples/agent-skills.md) for optional
agent skill examples. See [CHANGELOG.md](CHANGELOG.md) for release notes and
[docs/release-checklist.md](docs/release-checklist.md) for the manual release
checklist.

## Goals

- Capture notes quickly as canonical CommonMark files in the active vault.
- Keep a visible index of metadata, derived maps, and body terms.
- Filter by id, metadata, body text, date, collection, and links.
- Keep note files readable and editable without `nt`.
- Keep metadata visible in `$HOME/.nt/index.json`.
- Make search/filter speed a first-class design constraint.
- Stay flagless for core workflows.
- Compose cleanly with shell tools and completion.

## Core Loop

```text
capture -> index -> filter -> inspect -> connect -> revise
```

```sh
nt add [metadata...]
nt list
nt find <expr...>
nt show <id>
nt edit <id>
nt tags
nt collections
```

## Commands

```sh
nt init <notes-dir>
nt add [metadata...]
nt rebuild
nt list
nt find <expr...>
nt show <id>
nt edit <id>
nt rm <id>
nt ids
nt tags
nt tag <id> <tag>
nt untag <id> <tag>
nt collections
nt collection <name>
nt collect <id> <collection>
nt uncollect <id> <collection>
nt kind <id> <kind>
nt status
nt status <id> <status>
nt link <from-id> <to-id>
nt unlink <from-id> <to-id>
nt links <id> <out|in|self|all>
nt export <path> [id...]
nt config show
nt config vault
nt config vault <vault-name>
nt completion <shell>
nt help
nt help <command>
```

## Quick Start

```sh
nt init notes
printf '%s\n' '# First Note' '' 'body text' | nt add tag:example
nt find example
nt show NTYYYYMMDDTHHmmss
nt rebuild
```

Replace `NTYYYYMMDDTHHmmss` with the id printed by `nt add`.

For editor-first capture, run `nt add` to open `$EDITOR`, or
`nt add tag:example` to open `$EDITOR` and save the new note with that tag.
Update an existing note or its metadata with explicit commands:

```sh
nt edit NTYYYYMMDDTHHmmss
nt tag NTYYYYMMDDTHHmmss example
nt status NTYYYYMMDDTHHmmss open
nt collect NTYYYYMMDDTHHmmss projects/nt
```

Note files are flat CommonMark files:

```text
notes/
  NT20260528T143012.md
  NT20260528T150501.md
```

Metadata lives in `$HOME/.nt/index.json`, not Markdown front matter. Export can
generate interoperable front-matter copies without changing active notes:

```sh
nt export archive NT20260528T143012
```

Run `nt rebuild` after out-of-band file edits or deletes. It reconstructs the
active vault visible index from Markdown note files and visible JSON metadata:
it preserves primary metadata, preserves existing sources and merges URLs
currently found in Markdown body, removes stale active-vault entries, cleans
links to deleted notes, and refreshes the body term index.

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

## Search Philosophy

- Use exact metadata filters first.
- Use body term indexes for candidate narrowing before file scanning.
- Return deterministic active-recent results.
- Keep machine-facing output stable and one-record-per-line.
- Compose with normal shell tools for shell-first workflows.

## Shell-first Workflows

`nt` keeps the core loop to `nt find`, `nt show`, and `nt edit`. Paging, fuzzy
selection, previews, and batching come from shell tools such as `less`, `fzf`,
`awk`, and `xargs`.

See [docs/shell-workflows.md](docs/shell-workflows.md) for optional recipes. A
TUI is intentionally deferred and is not part of the current core.

## Agent Use

`nt` has no built-in agent command. That is deliberate. Any agent that can run
shell commands can use the same visible workflow:

```sh
nt help
nt list
nt tags
nt collections
nt find collection:projects/nt status:open
nt show NT20260528T143012
```

When an agent writes notes, it should draft CommonMark, ask before mutation when
appropriate, then save through `nt add` or edit through `nt edit`. Optional
skill examples live in [docs/examples/agent-skills.md](docs/examples/agent-skills.md)
so users can adapt them to Codex, Claude Code, Cursor, or any other agent
system without `nt` owning that runtime.

## Install from Source

```sh
cargo install --path .
```

For local development:

```sh
cargo build
cargo test
cargo clippy --all-targets
```
