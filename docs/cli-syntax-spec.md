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
- Machine-facing commands print one record per line.
- Mutations print one short lowercase status line.

## Commands

```sh
nt init <notes-dir>
nt add
nt list
nt find <expr...>
nt show <id>
nt edit <id>
nt discuss <id>
nt discuss <id> <prompt...>
nt rm <id>
nt rebuild
nt ids
nt tags
nt collections
nt collection <name>
nt collect <id> <collection>
nt uncollect <id> <collection>
nt kind <id> <kind>
nt status
nt status <id> <status>
nt link <from-id> <to-id>
nt unlink <from-id> <to-id>
nt links <id>
nt backlinks <id>
nt agent <prompt...>
nt config show
nt config agent-output <hidden|format|full>
nt completion <shell>
```

Avoid adding broader commands such as `search`, `grep`, `graph`, `open`, or
`browse` until real usage proves they are necessary.

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
nt find backlink:NT20260605T101500
nt find ref:firecracker
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
backlink:<id>          inbound link to id
ref:<term>             reference contains term
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
error: unknown query field `collectiom`; did you mean `collection`?
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
