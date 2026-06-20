# nt CLI Reference

This is the implemented command, query, value, and output contract for `nt`.
For a task-oriented introduction, see [usage.md](usage.md).

## Grammar

```text
nt <command> [positional...]
```

Core workflows use positional arguments rather than flags. Shell quoting keeps
multiword values in one argument. Note ids and filenames use:

```text
NTYYYYMMDDTHHmmss
NTYYYYMMDDTHHmmss.md
```

The top-level command surface is:

```text
nt init <notes-dir>
nt add [metadata...]
nt rebuild
nt list
nt list all [filter...]
nt list <field>[,<field>...] [filter...]
nt list ids
nt list titles
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
nt completion <bash|zsh>
nt help
nt help <command...>
```

Help and version flags are not part of the interface. Use `nt help`; package
version information comes from the normal distribution mechanism.

## init

```text
nt init <notes-dir>
```

Creates or opens a flat note directory, registers it under its basename, makes
it active, imports existing valid note files, and prints:

```text
initialized <vault-name> <path>
```

An existing directory must contain only regular files named
`NTYYYYMMDDTHHmmss.md`. A vault name must be unique in the index.

## add

```text
nt add [metadata...]
```

Reads CommonMark from stdin, or opens `$EDITOR` when stdin is a terminal. The
first non-empty line must be a non-empty `# Title` heading. Success prints
`saved <id>`.

Creation metadata:

| Expression | Meaning |
|---|---|
| `tag:<tag>[,<tag>...]` | Add one or more tags. Repeatable. |
| `kind:<kind>` | Set one kind. |
| `status:<status>` | Set one status. |
| `priority:<S|A|B|C|D>` | Set one priority. |
| `scheduled:<YYYY-MM-DD>` | Set one scheduled date. |
| `due:<YYYY-MM-DD>` | Set one due date. |
| `collection:<name>[,<name>...]` | Add one or more collections. Repeatable. |
| `link:<id>[,<id>...]` | Add outbound links to existing active notes. Repeatable. |
| `source:<value>` | Add one source string. Repeatable. Commas are literal. |

Single-value fields may appear only once. `closed` is system-managed and cannot
be supplied. Repeated values are deduplicated and stored in sorted order. URLs
found in the body are merged into sources.

```sh
printf '%s\n' '# Research' '' 'Compare runtimes.' \
  | nt add kind:research tag:qemu,firecracker collection:research/vm
```

## rebuild

```text
nt rebuild
```

Re-reads valid notes in the active vault and prints `rebuilt <count>`. It:

- derives `created` from the id and `updated` from the file timestamp
- validates and refreshes the title
- preserves existing primary metadata
- preserves existing sources and merges URLs currently found in Markdown bodies
- imports new valid files and removes stale active-vault entries
- removes links whose target no longer exists in the shared index
- rebuilds derived metadata, body, and heading indexes

## list

```text
nt list
nt list all [filter...]
nt list <field>[,<field>...] [filter...]
nt list ids
nt list titles
nt list tags [tag]
nt list collections [collection]
nt list links <id> [from|to]
```

`nt list` prints active notes in newest-created-first order. Bare `nt list`
prints this fixed summary:

```text
id, title, kind, status, due, tag
```

`nt list all` prints every indexed metadata field in this fixed order:

```text
id, path, created, updated, title, kind, status, priority, scheduled, due,
closed, tag, collection, link, source
```

Select one or more fields with a comma-separated first argument:

```sh
nt list id
nt list all status:done
nt list id,title,status
nt list title,tag
```

Rows contain no header. Columns are separated by one tab, set-like values are
comma-separated within their column, and absent optional or set-like values are
`-`. Paths are relative to the current directory when possible. Explicit
projections are the stable interface for scripts; fields added in future are
appended deliberately to the `all` projection.

After an optional projection, `list` accepts `AND`-combined structured filters:

```sh
nt list id,title,status status:open
nt list title,tag kind:decision since:2026-06-01 not:tag:draft
nt list status:waiting
```

Supported list filters are `id`, `tag`, `day`, `since`, `before`, `kind`,
`status`, `priority`, `scheduled`, `due`, `closed`, `collection`, `link`, and
`not` around another supported filter. Matching is case-insensitive and uses
the same validation, candidate narrowing, and active-recent ordering as `find`.
Bare words and `title`, `source`, and `body` expressions are search operations;
`list` rejects them and directs the user to `nt find`.

Compatibility and metadata operations:

| Form | Output |
|---|---|
| `list ids` | Compatibility alias for `list id`. |
| `list titles` | Compatibility alias for `list id,title`. |
| `list tags` | Sorted, deduplicated active tag names. |
| `list tags <tag>` | Matching summary records, newest first. |
| `list collections` | Sorted, deduplicated active collection names. |
| `list collections <name>` | Matching summary records, newest first. |
| `list links <id> from` | Existing outbound ids. |
| `list links <id> to` | Existing backlink ids. |
| `list links <id>` | Sorted, deduplicated union of both directions. |

## find

```text
nt find <expr...>
```

Every argument is one expression. Expressions are case-insensitive, combined
with `AND`, and independent of order. Matches print stable summary records in
active newest-first order. An empty result prints nothing and succeeds. Use
`list` when only structured filtering and metadata projection are needed; use
`find` for bare terms or title, source, and body search.

Expression forms:

| Expression | Match |
|---|---|
| `<word>` | Metadata contains the value or body contains its indexed term. |
| `#<tag>` | Exact tag; shorthand for `tag:<tag>`. |
| `id:<prefix>` | Id begins with a valid prefix of `NTYYYYMMDDTHHmmss`. |
| `tag:<tag>` | Exact tag. |
| `title:<term>` | Title contains the value. |
| `day:<YYYY-MM-DD>` | Created on the day. |
| `since:<YYYY-MM-DD>` | Created on or after the day. |
| `before:<YYYY-MM-DD>` | Created before the day. |
| `kind:<kind>` | Exact kind. |
| `status:<status>` | Exact status. |
| `priority:<S|A|B|C|D>` | Exact priority. |
| `scheduled:<YYYY-MM-DD>` | Exact scheduled date. |
| `due:<YYYY-MM-DD>` | Exact due date. |
| `closed:<YYYY-MM-DD>` | Closed during that UTC calendar day. |
| `collection:<name>` | Exact collection. |
| `link:<id>` | Has an outbound link to the exact id. |
| `source:<term>` | A source contains the value. |
| `body:<term>` | Body contains all tokenized terms. |
| `not:<expr>` | Negate one expression. |

Examples:

```sh
nt find qemu firecracker
nt find tag:decision collection:projects/nt
nt find since:2026-05-01 before:2026-06-01 not:tag:draft
nt find kind:todo priority:S due:2026-06-30
nt find body:'microvm jailer'
```

Shell quoting makes `body:microvm jailer` one argument in the last example.
Quoted multiword `body:` values match all indexed terms, not an exact phrase.
Indexed body entries are trusted until `nt rebuild`; notes missing from the
body trust set fall back to direct file reads. Unknown fields are errors, not
bare-word searches. There is no public `heading:<term>` field even though the
index contains rebuildable `heading_terms` for possible future use.

There is no ranking, fuzzy or semantic search, regex, parentheses, or `OR`.

## show

```text
nt show <id>
```

Prints identity and metadata, a blank line, then the exact CommonMark body:

```text
<id>  <title>
path <path>
created <UTC timestamp>
updated <UTC timestamp>
kind <kind>
status <value-or-dash>
priority <value-or-dash>
scheduled <value-or-dash>
due <value-or-dash>
closed <value-or-dash>
tags <comma-values-or-dash>
collections <comma-values-or-dash>
links <comma-values-or-dash>
sources <comma-values-or-dash>

# Title
...
```

## open

```text
nt open <id>
```

Copies the body to a temporary file, runs `$EDITOR` or `vi`, validates the
result, atomically replaces the canonical body, refreshes the title and text
indexes, merges body URLs into sources, and prints `saved <id>`. An empty body,
invalid title, or failed editor leaves the canonical note unchanged.

## rm

```text
nt rm <id>
```

Removes the active note, its indexed terms, and links to it, then prints
`removed <id>`. If saving the changed index fails, the body file is restored.

## update

```text
nt update <id> <field> <value>
```

Changes exactly one primary metadata field and prints
`updated <id> <field> <value>`.

Single-value fields:

| Field | Accepted value | Clear behavior |
|---|---|---|
| `kind` | A supported kind | `-` resets to `note`. |
| `status` | A supported status | `-` removes status and clears `closed`. |
| `priority` | `S`, `A`, `B`, `C`, or `D` | `-` removes priority. |
| `scheduled` | Valid `YYYY-MM-DD` | `-` removes the date. |
| `due` | Valid `YYYY-MM-DD` | `-` removes the date. |

Set-like fields require an operator:

| Field | Accepted value |
|---|---|
| `tag` | `+<tag>` or `-<tag>` |
| `collection` | `+<name>` or `-<name>` |
| `link` | `+<existing-id>` or `-<existing-id>` |
| `source` | `+<value>` or `-<value>` |

Adding an existing value and removing an absent value both succeed without
changing the set. A link target must exist in the active vault even for a
remove operation.

Changing status to `done` or `dropped` records the current UTC time in `closed`.
Repeating the same terminal status preserves that timestamp. Moving between
different terminal statuses records a new timestamp. `open`, `waiting`, and `-`
clear it.

## agenda

```text
nt agenda
nt agenda today
nt agenda week
nt agenda overdue
nt agenda waiting
nt agenda undated
```

Agenda includes only `kind:todo` notes with status `open` or `waiting`. The
default view prints non-empty section headings and assigns every included note
to exactly one section in this precedence:

| Section | Membership |
|---|---|
| `Overdue` | Open with `due` before today. |
| `Today` | Open with `due` today or `scheduled` on or before today. |
| `Upcoming` | Remaining dated open notes. |
| `Waiting` | All waiting notes, regardless of dates. |
| `Undated` | Remaining open notes with neither date. |

`today` includes Overdue and Today. `week` includes open overdue notes plus open
notes scheduled or due from today through the following six local calendar
days. The other views select their named section. Selected views omit headings.

Records are tab-separated:

```text
<id> <status> <priority-or-dash> <scheduled-or-dash> <due-or-dash> <title>
```

Dated sections sort by relevant date, then priority `S`, `A`, `B`, `C`, `D`,
then no priority, then active recency. The relevant date is `due` for Overdue
and the earliest available scheduled or due date for Today and Upcoming.
Waiting and Undated omit the date key.

## export

```text
nt export <path> [id...]
```

Exports all active notes in newest-first order, or the deduplicated ids in the
given order. The destination must be outside the active vault. Each
`<id>.md` copy is atomically written with generated front matter containing all
primary metadata, followed by the canonical body. Each success prints:

```text
exported <id> <path>
```

The active vault and index are unchanged.

## config

```text
nt config show
nt config vault
nt config vault <vault-name>
```

`config show` prints `vault <name-or-dash> <path-or-dash>`.

`config vault` lists known vaults sorted by name as:

```text
<star-if-active-or-dash> <name> <path>
```

Supplying a name selects it and prints `configured vault <name> <path>`.

## completion

```text
nt completion bash
nt completion zsh
```

Writes a completion script to stdout. Completion covers the typed command
grammar, note ids through `nt list id`, list fields and structured filters,
known metadata values, query prefixes,
and comma-separated creation metadata.

## help

```text
nt help
nt help <command...>
```

Prints root or command-specific help to stdout. Nested topics such as
`nt help config vault` are supported; unknown topics are errors.

## Values And Validation

Kinds:

```text
note todo meeting decision source research project
```

Statuses:

```text
open waiting done dropped
```

Priorities, highest first:

```text
S A B C D
```

Dates are real calendar dates in `YYYY-MM-DD`. Tags and collections are
non-empty lowercase values without whitespace or commas. `/` is allowed in a
collection name but does not create nested storage. Note ids must have the
documented shape; links must target existing active notes.

## Output And Errors

Successful mutations print one short line. Lists and `find` are one record per
line. Application errors are prefixed with `error:`, written to stderr, and exit
with status 1; `clap` rejects invalid command grammar before dispatch. ANSI
color appears only on TTY output and is disabled for redirected output,
`NO_COLOR`, or `TERM=dumb`.
