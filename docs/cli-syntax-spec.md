# nt CLI Syntax

This is the compact CLI command and query syntax contract for `nt`.

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
nt list
nt find <expr...>
nt show <id>
nt edit <id>
nt discuss <id>
nt discuss <id> <prompt...>
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
nt agent <prompt...>
nt config show
nt config vault
nt config vault <vault-name>
nt config agent-output <hidden|format|full>
nt completion <shell>
nt help
nt help <command>
```

Avoid adding broader commands such as `search`, `grep`, `graph`, `open`, or
`browse` until real usage proves they are necessary.

## Links

```text
nt links <id> <out|in|self|all>
```

Link modes:

```text
out                    direct outbound links from id
in                     direct inbound links to id
self                   direct inbound and outbound links
all                    graph walk through inbound and outbound links
```

`out` and `in` print one note id per line. `self` prints direct neighbors with a
direction prefix. `all` prints a deduplicated walk with distance and direction.
For `all`, direction is relative to the note expanded at the previous distance,
not always relative to the starting id.

Examples:

```sh
nt links NT20260605T101500 out
nt links NT20260605T101500 in
nt links NT20260605T101500 self
nt links NT20260605T101500 all
```

Example `self` output:

```text
out NT20260605T103000
in NT20260604T090000
```

Example `all` output:

```text
1 out NT20260605T103000
1 in NT20260604T090000
2 out NT20260606T120000
```

## Add

```text
nt add [metadata...]
```

`nt add` reads the note body from stdin, or opens `$EDITOR` when stdin is a
terminal. Optional metadata expressions attach visible JSON metadata while the
note is created, so the generated note id does not need to be known first.

Examples:

```sh
cat <<'EOF' | nt add tag:qemu kind:decision status:open collection:projects/nt
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
collection:<name>      add collection; comma-separated values are allowed
link:<id>              add outbound link; comma-separated ids are allowed
source:<term>          add one external source reference
```

`kind` and `status` accept one value. Repeat `tag`, `collection`, `link`, and
`source` expressions when multiple values are needed. `tag`, `collection`, and
`link` also accept comma-separated values. All `link:` target notes must already
exist.

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

## Find

```text
nt find <expr...>
```

Each `<expr>` is one query expression. All expressions are combined with `AND`.
Expression order does not matter. Search is case-insensitive.

Examples:

```sh
nt find qemu
nt find qemu firecracker
nt find tag:decision qemu
nt find kind:meeting status:open
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
word                   match searchable metadata or body
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
collection:<name>      exact collection
link:<id>              outbound link to id
source:<term>          source reference contains term
body:<term>            Markdown body contains term
not:<expr>             exclude simple expression
```

Quoted values rely on the shell:

```sh
nt find body:'microvm jailer'
```

`nt` receives `body:microvm jailer` as one argument. The query language does not
need a separate quoting syntax.

Unknown fields are errors, not bare-word searches:

```text
error: unknown query field `collectiom`
```

Avoid full boolean syntax, parentheses, scoring, fuzzy search, and regex by
default. If `OR` becomes necessary, add it later with explicit grouping instead
of overloading the v1 `AND` model.

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
