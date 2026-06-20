# Using nt

`nt` keeps canonical CommonMark notes in a flat vault and visible metadata in
`$HOME/.nt/index.json`. This guide covers the normal human and agent workflows.
See [cli-reference.md](cli-reference.md) for exact syntax, values, and output
contracts, and [design.md](design.md) for architecture and design decisions.

## Install And Initialize

Install from the repository and create a vault:

```sh
cargo install --path .
nt init notes
```

The vault name comes from the directory basename. A vault contains only files
named `NTYYYYMMDDTHHmmss.md`; subdirectories and other files are rejected.
Initializing an existing valid directory imports its notes into the index.

## Capture Notes

Pipe CommonMark to `nt add`:

```sh
cat <<'EOF' | nt add tag:storage kind:decision collection:projects/nt
# Keep metadata outside Markdown

The note body stays portable CommonMark.
EOF
```

The first non-empty line must be a non-empty `# Title` heading. When stdin is a
terminal, `nt add` opens `$EDITOR` instead. A successful add prints the new id:

```text
saved NT20260620T101500
```

Creation metadata can describe todos and relationships immediately:

```sh
cat <<'EOF' | nt add kind:todo status:open priority:A due:2026-06-30 tag:release link:NT20260620T101500
# Prepare the release

Run all release checks.
EOF
```

Repeat `tag:`, `collection:`, `link:`, and `source:` for multiple values. Tags,
collections, and links also accept comma-separated values. URLs in the body are
automatically added to the note's source metadata.

## Find And Read

Start with cheap visible projections, then narrow the result:

```sh
nt list
nt list tags
nt list collections
nt find kind:decision tag:storage
nt find since:2026-06-01 body:'metadata CommonMark'
nt show NT20260620T101500
```

Every `find` expression is combined with `AND`; order does not matter and
matching is case-insensitive. Bare words search metadata and body terms. Use
exact metadata fields when possible. Quoted multiword `body:` values match all
indexed terms, not an exact phrase.

`nt find` narrows candidates with the visible metadata and body indexes, then
prints verified matches in deterministic newest-first order. Indexed body
entries are trusted until `nt rebuild`; out-of-band Markdown edits are not
visible to indexed body search until the index is rebuilt.

Use normal shell tools for paging, selection, previews, and batching:

```sh
nt find rust | less
nt find rust | fzf --preview 'nt show {1}'
nt find rust | fzf | awk '{print $1}' | xargs nt open
nt list ids | fzf --multi | xargs -n1 nt show
```

This shell-first workflow is the interactive interface. A TUI is intentionally
deferred and is not part of the current core.

## Organize Notes

Change one metadata field at a time:

```sh
nt update NT20260620T101500 kind project
nt update NT20260620T101500 status open
nt update NT20260620T101500 tag +storage
nt update NT20260620T101500 collection +projects/nt
nt update NT20260620T101500 link +NT20260619T090000
nt update NT20260620T101500 source +https://example.com/spec
```

Set-like fields require `+value` or `-value`, making repeated updates
idempotent. Single-value fields take a plain value and use `-` to clear; clearing
`kind` resets it to `note`.

Inspect relationships with:

```sh
nt list links NT20260620T101500
nt list links NT20260620T101500 from
nt list links NT20260620T101500 to
```

Links are metadata, not special Markdown syntax. `from` means outbound links;
`to` means backlinks.

## Work With Todos

Todos use `kind:todo` and an actionable `status`:

```sh
nt update NT20260620T101500 kind todo
nt update NT20260620T101500 status open
nt update NT20260620T101500 priority S
nt update NT20260620T101500 scheduled 2026-06-25
nt update NT20260620T101500 due 2026-06-30
```

View actionable work:

```sh
nt agenda
nt agenda today
nt agenda week
nt agenda overdue
nt agenda waiting
nt agenda undated
```

The default agenda groups each open or waiting todo once under `Overdue`,
`Today`, `Upcoming`, `Waiting`, or `Undated`. Priorities sort from `S` through
`D`. Setting status to `done` or `dropped` records a UTC `closed` timestamp;
reopening the note clears it.

## Edit, Remove, And Rebuild

```sh
nt open NT20260620T101500
nt rm NT20260620T101500
nt rebuild
```

`open` edits through `$EDITOR`, validates the title, writes the body atomically,
and refreshes the index. `rm` removes the body, metadata, body terms, and inbound
links.

Run `nt rebuild` after editing, adding, or deleting vault files outside `nt`.
It preserves primary JSON metadata, preserves existing sources and merges URLs
currently found in Markdown bodies, removes stale active-vault entries, cleans
dangling links, and refreshes titles, file timestamps, and text indexes.

## Export And Vaults

Export all active notes or selected ids to a directory outside the active
vault:

```sh
nt export archive
nt export archive NT20260620T101500 NT20260619T090000
```

Exported copies contain generated front matter. The canonical vault files stay
plain CommonMark and are not modified.

Inspect and switch known vaults:

```sh
nt config show
nt config vault
nt config vault work
```

## Completion And Help

Generate completion for Bash or Zsh:

```sh
nt completion bash
nt completion zsh
```

The generated scripts complete commands, note ids, metadata, query fields, and
known tags and collections. Use built-in positional help instead of flags:

```sh
nt help
nt help find
nt help config vault
```

## Agent Workflow

Agents use the same commands and storage as humans:

```sh
nt list tags
nt list collections
nt find collection:projects/nt status:open
nt show NT20260620T101500
```

When answering from notes, cite note ids. Before mutations, draft the note or
show the exact `nt update` commands and obtain approval. There is no hidden
agent memory, agent-only command, launcher, or retrieval path.
