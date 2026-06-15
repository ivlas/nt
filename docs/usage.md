# nt Usage Guide

`nt` is a Markdown-first, Git-friendly personal knowledge index for humans and
agents. Notes are plain Markdown files, while metadata lives in
`$HOME/.nt/index.json`.

The core goal is `time-to-knowledge`: the shortest path from vague memory to an
exact note id and the note content behind it.

See [cli-syntax-spec.md](cli-syntax-spec.md) for the compact command and query
syntax contract.

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
cat <<'EOF' | nt add tag:storage kind:decision status:open collection:projects/nt
# Storage decision

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
nt ids
nt tags
nt collections
nt find storage
nt find since:2026-05-01 before:2026-06-01 tag:decision
nt show NT20260528T143012
```

Use `nt show <id>` for exact retrieval. It prints identity and metadata before
the CommonMark body.

Search/filter speed is a first-class design constraint. Start with exact
metadata filters when possible. `nt find` uses visible body term indexes in
`$HOME/.nt/index.json` where available, while shell file scanning remains the
fallback for ad hoc inspection. Quoted multiword `body:` values match all
indexed terms, not an exact phrase.

## Rebuild Metadata

```sh
nt rebuild
```

`nt rebuild` scans the active vault's valid note files, refreshes title and
updated metadata, preserves existing sources and merges URLs currently found in
Markdown body, removes stale active-vault entries, cleans links to deleted
notes, rebuilds derived maps and text term indexes, and prints
`rebuilt <count>`.

## Search Philosophy

- Exact metadata filters first.
- Use indexed text search before file scanning.
- Deterministic results.
- Stable one-record-per-line output.
- Shell composition.

## Organize Metadata

```sh
nt collect NT20260528T143012 projects/nt
nt uncollect NT20260528T143012 projects/nt
nt tag NT20260528T143012 storage
nt untag NT20260528T143012 storage
nt kind NT20260528T143012 decision
nt status NT20260528T143012 open
nt status NT20260528T143012 -
nt link NT20260528T143012 NT20260527T120000
nt unlink NT20260528T143012 NT20260527T120000
```

List open and waiting notes:

```sh
nt status
```

Inspect collections and links:

```sh
nt collection projects/nt
nt links NT20260528T143012 out
nt links NT20260528T143012 in
nt links NT20260528T143012 self
nt links NT20260528T143012 all
```

## Edit And Remove

```sh
nt edit NT20260528T143012
nt rm NT20260528T143012
```

`nt edit` opens `$EDITOR`, saves the Markdown body atomically, and refreshes
the visible title metadata.

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
visible `nt ids` output. It also completes query and metadata expressions such
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
nt tags
nt collections
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

```sh
nt ids | head
nt find meeting | awk '{print $1}'
nt tags | sort
nt collections | sort
```
