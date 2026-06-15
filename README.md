# nt

`nt` is a Markdown-first, Git-friendly personal knowledge index: plain
Markdown notes, visible JSON metadata, deterministic search, and shell-friendly
commands. Its core goal is `time-to-knowledge`: the shortest path from vague
memory to an exact note id and the note content behind it.

It is intentionally useful to both humans and coding agents because it behaves
like a normal Unix tool. It reads stdin, writes stdout, opens `$EDITOR`, exposes
stable one-record-per-line commands, and does not keep a hidden memory layer.

`nt` keeps notes as plain Markdown and metadata as visible JSON. It is not an
app framework, agent runtime, RAG system, vector database, daemon, server,
browser/runtime orchestrator, workflow engine, or launcher for a specific
agent. Agents can still use it directly through zsh/bash by reading `nt help`,
running `nt find`, and inspecting exact notes with `nt show`.

See [docs/usage.md](docs/usage.md) for a compact guide,
[docs/cli-syntax-spec.md](docs/cli-syntax-spec.md) for the command/query
contract, [docs/design.md](docs/design.md) for boundaries, and
[docs/examples/agent-skills.md](docs/examples/agent-skills.md) for optional
agent skill examples.

## Goals

- Capture notes quickly as canonical CommonMark files.
- Index visible metadata and rebuildable derived maps.
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

cat <<'EOF' | nt add tag:storage kind:decision status:open collection:projects/nt
# Storage shape

Keep note metadata outside Markdown.
EOF

nt list
nt find tag:storage
nt show NT20260528T143012
nt edit NT20260528T143012
nt rebuild
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

Run `nt rebuild` to reconstruct active-vault metadata from Markdown note files
and visible JSON metadata after out-of-band file edits or deletes.

## Search

`nt find` takes positional query expressions. All expressions are combined with
`AND`; order does not matter; search is case-insensitive.

```sh
nt find qemu firecracker
nt find tag:decision collection:projects/nt
nt find since:2026-05-01 before:2026-06-01 not:tag:draft
nt find body:'microvm jailer'
```

Common expressions:

```text
qemu                  metadata or body contains qemu
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
body:'microvm jailer'
not:tag:draft
```

Unknown fields are errors so typos do not silently become broad text searches.

## Search Philosophy

- Use exact metadata filters first.
- Evolve toward indexed text search before file scanning.
- Return deterministic results.
- Keep machine-facing output stable and one-record-per-line.
- Compose with normal shell tools.

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

## Development

```sh
cargo fmt
cargo test
cargo run -- help
```
