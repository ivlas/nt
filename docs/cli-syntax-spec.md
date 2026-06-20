# nt CLI Syntax

This is the compact CLI command and query syntax contract for `nt`, a
Markdown-first personal knowledge index optimized for `time-to-knowledge`: the
shortest path from vague memory to an exact note id and the note content behind
it.

This document defines the current command contract. The consolidated `list`,
`update`, and `agenda` commands replace the legacy metadata command surface.

## General Form

```text
nt <command> [positional...]
```

Rules:

- Core workflows use no flags.
- Arguments are positional.
- Multiword user text uses trailing positionals.
- Multiword query values use normal shell quoting.
- Note ids use `NTYYYYMMDDTHHmmss`.
- Note filenames use `<id>.md`.
- Machine-facing commands print stable records, one record per line.
- Mutations print one short lowercase status line.

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

## Rebuild

```text
nt rebuild
```

`nt rebuild` reconstructs the active vault visible index from valid
`NTYYYYMMDDTHHmmss.md` files and visible JSON metadata. It preserves primary
metadata, refreshes titles and file `updated` times, preserves existing sources
and merges URLs currently found in Markdown body, removes stale active-vault
entries, cleans links to deleted notes, rebuilds derived maps and text term
indexes including the body term index, and prints `rebuilt <count>`.

Avoid adding broader commands such as `search`, `grep`, `graph`,
`browse`, `agent`, or `discuss` until real usage proves they belong in `nt`
itself. `nt` keeps notes as Markdown and metadata as JSON. It is not an app
framework, agent runtime, or vector/RAG system. Agent integrations should live
outside `nt` as docs, skills, shell wrappers, or agent-specific configuration.
A TUI is intentionally deferred and is not part of the current core.

## List

```text
nt list
nt list ids
nt list tags [tag]
nt list collections [collection]
nt list links <id> [from|to]
```

`nt list` prints all active-vault notes in active-recent order using the normal
`id, date, tags, title` summary format. Its submodes provide stable,
one-record-per-line output:

```text
ids                          note id
tags                         available tag
tags <tag>                   notes with tag
collections                  available collection
collections <collection>     notes in collection
links <id> [from|to]         related note ids
```

Unfiltered `tags` and `collections` print sorted, deduplicated values from the
active vault. Supplying a value prints matching notes in active-recent order
using the normal note summary format.

Link directions:

```text
no direction           all directly related note ids
from                   notes this note links to
to                     notes linking to this note
```

Every links form prints one note id per line. The default view deduplicates
inbound and outbound relationships.

Examples:

```sh
nt list links NT20260605T101500
nt list links NT20260605T101500 from
nt list links NT20260605T101500 to
nt list tags storage
nt list collections projects/nt
```

Example default output:

```text
NT20260604T090000
NT20260605T103000
```

## Export

```text
nt export <path> [id...]
```

`nt export` writes Markdown copies into `<path>` with generated front matter
that mirrors the current note metadata from `$HOME/.nt/index.json`. The active
notes directory remains plain CommonMark and is not modified.

If no ids are provided, all notes in the active vault are exported. If ids are
provided, only those notes are exported. Exported filenames use `<id>.md`.

Examples:

```sh
nt export archive
nt export archive NT20260528T143012
nt export archive NT20260528T143012 NT20260527T120000
nt find collection:projects/nt | awk '{print $1}' | while read -r id; do nt export archive "$id"; done
```

Example exported file:

```markdown
---
id: "NT20260528T143012"
path: "/home/me/notes/NT20260528T143012.md"
created: "2026-05-28T14:30:12Z"
updated: "2026-05-28T14:30:12Z"
title: "Storage shape"
kind: "decision"
status: "open"
priority: "S"
scheduled: "2026-06-25"
due: "2026-06-30"
closed: null
tags: ["storage"]
collections: ["projects/nt"]
links: []
sources: []
---

# Storage shape

Keep the note format simple.
```

## Add

```text
nt add [metadata...]
```

`nt add` reads the note body from stdin, or opens `$EDITOR` when stdin is a
terminal. Optional metadata expressions attach visible JSON metadata while the
note is created, so the generated note id does not need to be known first.
The first non-empty body line must be a non-empty `# Title` heading. The
Markdown heading is the source of truth for required indexed title metadata.

Examples:

```sh
cat <<'EOF' | nt add tag:qemu kind:todo status:open priority:S scheduled:2026-06-25 due:2026-06-30 collection:projects/nt
# Note Body

Text.
EOF

cat <<'EOF' | nt add tag:qemu,firecracker tag:research
# VM research

Compare QEMU and Firecracker.
EOF

cat <<'EOF' | nt add link:NT20260605T101500,NT20260605T103000 tag:followup
# Follow-up

Connect this note to two earlier notes.
EOF
```

Creation metadata fields:

```text
tag:<tag>              add tag; comma-separated values are allowed
kind:<kind>            set one kind
status:<status>        set one status
priority:<priority>    set one priority: S, A, B, C, or D
scheduled:<YYYY-MM-DD> set one scheduled date
collection:<name>      add collection; comma-separated values are allowed
link:<id>              add outbound link; comma-separated ids are allowed
source:<term>          add one external source reference
due:<YYYY-MM-DD>       set one due date
```

`kind`, `status`, `priority`, `scheduled`, and `due` accept one value. Repeat
`tag`, `collection`, `link`, and `source` expressions when multiple values are
needed. `tag`, `collection`, and `link` also accept comma-separated values. All
`link:` target notes must already exist. `closed` is system-managed and cannot
be supplied to `add`.

These tag forms are equivalent:

```sh
nt add tag:qemu,firecracker tag:research kind:decision
nt add tag:qemu,firecracker,research kind:decision
nt add tag:qemu tag:firecracker tag:research kind:decision
```

These link forms are equivalent:

```sh
nt add link:NT20260605T101500,NT20260605T103000
nt add link:NT20260605T101500 link:NT20260605T103000
```

Repeat `source:` for multiple source references:

```sh
nt add source:https://a.example/spec source:https://b.example/spec
```

## Update

```text
nt update <id> <field> <value>
```

`update` is the single metadata mutation command. Single-value fields take a
plain value and use `-` to clear it. Set-like fields require an explicit `+` or
`-` prefix so updates are idempotent and do not silently toggle state.

```text
kind          <kind> or -
status        <status> or -
priority      S, A, B, C, D, or -
scheduled     <YYYY-MM-DD> or -
due           <YYYY-MM-DD> or -
tag           +<tag> or -<tag>
collection    +<name> or -<name>
link          +<id> or -<id>
source        +<term> or -<term>
```

Examples:

```sh
nt update NT20260528T143012 kind todo
nt update NT20260528T143012 status open
nt update NT20260528T143012 priority S
nt update NT20260528T143012 scheduled 2026-06-25
nt update NT20260528T143012 due 2026-06-30
nt update NT20260528T143012 tag +nt
nt update NT20260528T143012 collection +projects/nt
nt update NT20260528T143012 link +NT20260527T120000
nt update NT20260528T143012 tag -draft
nt update NT20260528T143012 due -
```

Each invocation changes exactly one field value and writes the index atomically.
Successful updates print `updated <id> <field> <value>`.

## Agenda

```text
nt agenda
nt agenda today
nt agenda week
nt agenda overdue
nt agenda waiting
nt agenda undated
```

`agenda` is a read-only view of actionable todo notes. It includes notes whose
kind is `todo` and whose status is `open` or `waiting`.

The default view partitions each note into exactly one section, in this
precedence order:

```text
Overdue    open notes whose due date is before today
Today      open notes due today or scheduled on or before today
Upcoming   remaining dated open notes
Waiting    all waiting notes
Undated    open notes with neither scheduled nor due
```

`today` prints Overdue and Today. `week` prints overdue items plus items due or
scheduled from today through the following six calendar days. The other
selectors print their matching section. Dates use the local calendar day.

Each record contains `id, status, priority, scheduled, due, title`; absent
values print `-`. Overdue uses due as its relevant date. Today and Upcoming use
the earliest scheduled or due date that places the note in that section. Dated
sections sort by relevant date ascending, then by priority (`S`, `A`, `B`, `C`,
`D`, then no priority), then by active-recent order. Waiting and Undated sort by
priority and then active-recent order.

When status changes to `done` or `dropped`, `nt` records the current UTC time in
`closed`. Repeating the same terminal status preserves the original timestamp.
Changing status to `open`, `waiting`, or `-` clears `closed`.

The first version does not parse Markdown task syntax and does not add
recurrence, effort estimates, time tracking, or habits.

Completion may complete comma-separated metadata values when the expression
stays one shell word:

```sh
nt add tag:qemu,fire<TAB>
```

## Find

```text
nt find <expr...>
```

Each `<expr>` is one query expression. All expressions are combined with `AND`.
Expression order does not matter. Search is case-insensitive. `nt find` uses
the visible index in `$HOME/.nt/index.json`, including metadata maps and the
body term index, for candidate narrowing where available. Markdown file scans
are reserved for missing body index entries. Indexed body entries are trusted
until `nt rebuild` refreshes them. Final results are still printed in
deterministic active-recent order, with no ranking, fuzzy search, or semantic
search. Quoted multiword `body:` values match all indexed terms, not an exact
phrase.
The visible `heading_terms` index is for future/internal use; there is no
`heading:<term>` query field yet.

Examples:

```sh
nt find qemu
nt find qemu firecracker
nt find tag:decision qemu
nt find kind:meeting status:open
nt find kind:todo due:2026-06-30
nt find kind:todo scheduled:2026-06-25 priority:S
nt find collection:projects/nt
nt find since:2026-05-01 before:2026-06-01 tag:decision collection:projects/nt
nt find link:NT20260605T101500
nt find source:firecracker
nt find body:'microvm jailer'
nt find not:tag:draft qemu
```

The combined date/tag/collection example means:

```text
created on or after 2026-05-01
AND created before 2026-06-01
AND has tag decision
AND belongs to collection projects/nt
```

## Query Expressions

```text
word                   match searchable metadata or indexed body terms
#tag                   exact tag match
field:value            field predicate
not:expr               negate one simple expression
```

Initial fields:

```text
id:<id>                exact or prefix note id
tag:<tag>              exact tag
title:<term>           title contains term
day:<YYYY-MM-DD>       created on day
since:<YYYY-MM-DD>     created on or after day
before:<YYYY-MM-DD>    created before day
kind:<kind>            exact kind
status:<status>        exact status
priority:<priority>    exact priority
scheduled:<YYYY-MM-DD> exact scheduled date
due:<YYYY-MM-DD>       exact due date
closed:<YYYY-MM-DD>    closed during the UTC calendar day
collection:<name>      exact collection
link:<id>              outbound link to id
source:<term>          source reference contains term
body:<term>            indexed Markdown body contains all terms
not:<expr>             exclude simple expression
```

Quoted values rely on the shell:

```sh
nt find body:'microvm jailer'
```

`nt` receives `body:microvm jailer` as one argument. The query language does not
need a separate quoting syntax. This matches notes containing both indexed terms
`microvm` and `jailer`; it does not require the exact phrase `microvm jailer`.

Unknown fields are errors, not bare-word searches:

```text
error: unknown query field `collectiom`
```

Avoid full boolean syntax, parentheses, scoring, fuzzy search, and regex by
default. If `OR` becomes necessary, add it later with explicit grouping instead
of overloading the v1 `AND` model.

## Search Philosophy

- Exact metadata filters first.
- Body term index candidate narrowing before file scanning.
- Deterministic active-recent results.
- Stable one-record-per-line output.
- Shell-first workflows for ad hoc expansion.

## Values

Initial kinds:

```text
note
todo
meeting
decision
source
research
project
```

Initial statuses:

```text
open
waiting
done
dropped
```

Use `nt update <id> <field> -` to clear a user-settable single-value field such
as status, priority, scheduled, or due.

Priorities, highest to lowest:

```text
S
A
B
C
D
```

Completion shells:

```text
bash
zsh
```

## Unix Tools

`nt find` owns structured note predicates. Keep `rg`, `fd`, `bat`, `awk`, and
other shell tools outside the core syntax:

```sh
nt find qemu | awk '{print $1}'
nt show NT20260605T101500 | bat -l markdown
rg -n firecracker ~/nt/notes
fd -e md . ~/nt/notes
```
