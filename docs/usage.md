# nt Usage Guide

`nt` is a Markdown-first, Git-friendly personal knowledge index for humans and
agents. Notes are plain Markdown files, while metadata lives in
`$HOME/.nt/index.json`.

The core goal is `time-to-knowledge`: the shortest path from vague memory to an
exact note id and the note content behind it.

See [cli-syntax-spec.md](cli-syntax-spec.md) for the compact command and query
syntax contract.

This guide describes the current consolidated command surface.

## Setup

Create a vault from a notes directory:

```sh
nt init notes
```

The vault name is the directory basename and must be unique. The notes directory
is flat and contains only `<id>.md` note files.

## Add Notes

Add a Markdown note from stdin:

```sh
cat <<'EOF' | nt add
# Storage decision

Keep note metadata outside Markdown.
EOF
```

Attach metadata while creating the note:

```sh
cat <<'EOF' | nt add tag:storage kind:todo status:open priority:S scheduled:2026-06-25 due:2026-06-30 collection:projects/nt
# Finish storage design

Keep note metadata outside Markdown.
EOF
```

Repeated metadata fields and comma-separated values are equivalent:

```sh
nt add tag:qemu,firecracker tag:research kind:decision
nt add tag:qemu,firecracker,research kind:decision
nt add tag:qemu tag:firecracker tag:research kind:decision
```

Link the new note to existing notes during creation:

```sh
cat <<'EOF' | nt add link:NT20260605T101500,NT20260605T103000 tag:followup
# Follow-up

Connect this note to two earlier notes.
EOF
```

If stdin is a terminal, `nt add` opens `$EDITOR`.

## Filter And Read

```sh
nt list
nt list ids
nt list tags
nt list collections
nt list tags storage
nt list collections projects/nt
nt find storage
nt find since:2026-05-01 before:2026-06-01 tag:decision
nt show NT20260528T143012
```

Use `nt show <id>` for exact retrieval. It prints identity and metadata before
the CommonMark body.

Search/filter speed is a first-class design constraint. Start with exact
metadata filters when possible. `nt find` uses the visible index in
`$HOME/.nt/index.json`, including metadata maps and the body term index, for
candidate narrowing where available, then prints verified results in
active-recent order. There is no ranking, fuzzy search, or semantic search.
Markdown file scans are reserved for notes missing from `body_indexed`; indexed
body entries are trusted until `nt rebuild` refreshes them. Shell file scanning
remains the fallback for ad hoc inspection. Quoted multiword `body:` values
match all indexed terms, not an exact phrase.

## Rebuild Metadata

```sh
nt rebuild
```

`nt rebuild` scans the active vault's valid note files, refreshes title and
updated metadata, preserves existing sources and merges URLs currently found in
Markdown body, removes stale active-vault entries, cleans links to deleted
notes, rebuilds derived maps and the body term index, and prints
`rebuilt <count>`.

## Search Philosophy

- Exact metadata filters first.
- Use the body term index for candidate narrowing before file scanning.
- Deterministic active-recent results.
- Stable one-record-per-line output.
- Shell-first workflows.

## Organize Metadata

```sh
nt update NT20260528T143012 collection +projects/nt
nt update NT20260528T143012 collection -projects/nt
nt update NT20260528T143012 tag +storage
nt update NT20260528T143012 tag -storage
nt update NT20260528T143012 kind todo
nt update NT20260528T143012 status open
nt update NT20260528T143012 priority S
nt update NT20260528T143012 scheduled 2026-06-25
nt update NT20260528T143012 due 2026-06-30
nt update NT20260528T143012 link +NT20260527T120000
nt update NT20260528T143012 link -NT20260527T120000
```

Single-value fields use `-` to clear them. Set-like fields require `+value` or
`-value`, so repeating a command is safe and never toggles metadata implicitly.

Show actionable todo notes in Overdue, Today, Upcoming, Waiting, and Undated
sections:

```sh
nt agenda
nt agenda today
nt agenda week
nt agenda overdue
nt agenda waiting
nt agenda undated
```

Agenda rows show status, priority, scheduled date, and due date. Priorities are
`S`, `A`, `B`, `C`, and `D`, highest to lowest. Marking a note `done` or
`dropped` records a `closed` UTC timestamp; reopening it clears that timestamp.

Inspect collections and links:

```sh
nt find collection:projects/nt
nt list links NT20260528T143012
nt list links NT20260528T143012 from
nt list links NT20260528T143012 to
```

## Edit And Remove

```sh
nt open NT20260528T143012
nt rm NT20260528T143012
```

`nt open` opens `$EDITOR`, validates the required `# Title` heading, saves the
Markdown body atomically, and refreshes the visible title metadata.

## Export

Export active notes as Markdown copies with generated front matter:

```sh
nt export archive
nt export archive NT20260528T143012
nt export archive NT20260528T143012 NT20260527T120000
```

Export query results with normal shell composition:

```sh
nt find since:2026-05-01 before:2026-06-01 collection:projects/nt \
  | awk '{print $1}' \
  | while read -r id; do nt export archive "$id"; done
```

The active note files stay plain Markdown. `$HOME/.nt/index.json` remains the
metadata source of truth.

## Vaults

```sh
nt config show
nt config vault
nt config vault notes
```

`nt config show` prints the active vault. `nt config vault` lists known vaults,
and `nt config vault <vault-name>` switches the active vault.

## Completion And Help

```sh
nt completion zsh
nt completion bash
nt help
nt help find
nt help config vault
```

Completion uses `clap_complete` and dynamic note id completion backed by
visible `nt list ids` output. It also completes query and metadata expressions such
as `tag:`, `status:`, `collection:`, and comma-separated values:

```sh
nt find sta<TAB>
nt find tag:<TAB>
nt add tag:qemu,fire<TAB>
```

Keep comma-separated metadata in one shell word, without a space after the
comma.

## Agent Use

Agents should use the same visible commands as humans:

```sh
nt help
nt list
nt list tags
nt list collections
nt agenda
nt find meeting
nt show NT20260528T143012
```

When answering from notes, cite supporting note ids. When writing notes, draft
CommonMark and save through `nt add`; update metadata with explicit commands.

There is no `nt agent`, `nt discuss`, built-in skill installer, hidden
retrieval, embedding store, daemon, app framework, agent runtime, vector/RAG
system, or agent-specific behavior. Active notes are Markdown files, and
metadata is JSON. Optional skill examples are documentation only:
[examples/agent-skills.md](examples/agent-skills.md).

## Unix Composition

Shell-first workflows keep paging, previews, fuzzy selection, and batching
outside the core command surface. A TUI is intentionally deferred and is not
part of the current core.

```sh
nt list ids | head
nt find meeting | awk '{print $1}'
nt list tags
nt list collections
```
