# Using nt

`nt` keeps canonical CommonMark notes in a flat vault and visible metadata in
`$HOME/.nt/index.json`. This guide covers the user-owned workflow and how an
agent can assist on the user's direction.
See [cli-reference.md](cli-reference.md) for exact syntax, values, and output
contracts, and [design.md](design.md) for architecture and design decisions.

## User-Directed Use

`nt` is the user's note organizer. The user can use the CLI directly or ask
an agent to run a specific command, but an agent does not decide to capture,
change, or remove notes on its own. Most agent work should be read-only
retrieval: list, find, and show commands that help the user inspect their
notes.

Mutating commands assume one user-directed writer at a time; do not run them
concurrently. Note bodies are plain Markdown and metadata is plain JSON, so
editing note files outside `nt` is always safe: body search reads Markdown
bodies at query time, and out-of-band edits are visible to search immediately.
If a write fails after changing a note file, inspect the visible files and
reapply any explicit metadata that did not reach the index with `nt update`.

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

Pipe CommonMark to `nt note`:

```sh
cat <<'EOF' | nt note tag:storage,decision collection:projects/nt
# Keep metadata outside Markdown

The note body stays portable CommonMark.
EOF
```

The first non-empty line must be a non-empty `# Title` heading. When stdin is a
terminal, `nt note` opens `$EDITOR` instead. A successful save prints the new id:

```text
saved NT20260620T101500
```

Use `nt todo` for actionable notes:

```sh
cat <<'EOF' | nt todo priority:A due:2026-06-30 tag:release link:NT20260620T101500
# Prepare the release

Run all release checks.
EOF
```

Repeat `tag:`, `collection:`, `link:`, and `source:` for multiple values. Tags,
collections, and links also accept comma-separated values. URLs in the body are
automatically added to the note's source metadata. New todos default to
`status:open`; pass `status:<status>` to create a waiting, done, or dropped
todo directly.

## Find And Read

Start with cheap visible projections, then narrow the result:

```sh
nt list
nt list all status:done
nt list id,title,status status:open
nt list title,tag tag:decision
nt list tags
nt list collections
nt list sources
nt find tag:decision tag:storage
nt find since:2026-06-01 body:'metadata CommonMark'
nt show NT20260620T101500
```

Bare `nt list` prints `id`, `title`, `kind`, `status`, `due`, and `tag` with a
header and aligned columns in a terminal. Redirected output is headerless and
tab-separated. `nt list all` prints every metadata field. A comma-separated
first argument selects custom fields; following arguments use the exact
structured subset of the `find` grammar. Use `find` for bare words and title,
source, or body search.

Every `find` expression is combined with `AND`; order does not matter and
matching is case-insensitive. Bare words search metadata and body text. Use
exact metadata fields when possible. Quoted multiword `body:` values match all
terms, not an exact phrase.

`nt find` evaluates expressions against the visible JSON metadata and prints
matches in deterministic newest-first order. Body search reads Markdown bodies
at query time; out-of-band edits are visible to search immediately.

Use normal shell tools for paging, selection, previews, and batching:

```sh
nt find rust | less
nt find rust | fzf --preview 'nt show {1}'
nt find rust | fzf | awk '{print $1}' | xargs nt open
nt list id | fzf --multi | xargs -n1 nt show
```

This shell-first workflow is the interactive interface. A TUI is intentionally
deferred and is not part of the current core.

## Organize Notes

Change one metadata field at a time:

```sh
nt update NT20260620T101500 tag +storage
nt update NT20260620T101500 collection +projects/nt
nt update NT20260620T101500 link +NT20260619T090000
nt update NT20260620T101500 source +https://example.com/spec
```

Set-like fields require `+value` or `-value`, making repeated updates
idempotent. Single-value fields take a plain value and use `-` to clear; clearing
`kind` resets it to `note`. Todo-only fields such as `status`, `priority`,
`scheduled`, and `due` can be set only on todo notes.

Inspect relationships with:

```sh
nt list links from:NT20260620T101500
nt list links to:NT20260620T101500
```

Links are unlabeled directed metadata, not special Markdown syntax.
`from:<id>` selects outbound links and `to:<id>` selects backlinks. State the
relationship meaning, such as "Follow-up to" or "Supersedes", in the note's
ordinary CommonMark body. Use metadata for the connection and prose for its
meaning; do not overload titles, tags, or collections with link types.

## Work With Todos

Todos use `nt todo` at creation time and an actionable `status`:

```sh
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

## Edit And Remove

```sh
nt open NT20260620T101500
nt rm NT20260620T101500
```

`open` edits through `$EDITOR`, validates the title, writes the body atomically,
and updates the stored title and timestamp. `rm` removes the body, its metadata,
and inbound links.

Editing note bodies outside `nt` needs no reconciliation: body search reads
the current file contents. If you delete a note file out-of-band, remove its
metadata with `nt rm <id>`; until then, body search reports the missing file.

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

## User-Directed Agent Use

Agents use the same commands and storage as users, only when the user asks:

```sh
nt list tags
nt list collections
nt list sources
nt list id,title,status status:open
nt find collection:projects/nt status:open
nt show NT20260620T101500
```

When answering from notes, cite note ids. Before mutations, draft the note or
show the exact `nt update` commands and obtain approval. There is no hidden
agent memory, agent-owned note workflow, agent-only command, launcher, or
retrieval path.
