# nt Design

`nt` is a Markdown-first, Git-friendly personal knowledge index for humans and
agents. It is built around visible commands, deterministic retrieval, editable
CommonMark notes, and no hidden memory layer.

The core product goal is `time-to-knowledge`: the shortest path from vague
memory to an exact note id and the note content behind it. In practice, this
means a user or agent should be able to start with partial memory, apply cheap
filters, inspect a small candidate set, and land on the right Markdown note
quickly.

Humans and agents use the same Unix-like interface: stdin, stdout, `$EDITOR`,
plain files, and normal shell composition.

## Boundaries

`nt` keeps notes as plain Markdown and metadata as visible JSON. It is not an
app framework, agent runtime, RAG system, vector database, daemon, server,
browser/runtime orchestrator, microVM orchestrator, workflow engine, or Hermes
replacement.

Core behavior must not add hidden retrieval, embeddings, background state,
agent-only paths, or framework complexity.

## Core Loop

The core workflow is:

```text
capture -> index -> filter -> inspect -> connect -> revise
```

It maps to small commands such as:

```sh
nt add [metadata...]
nt list
nt find <expr...>
nt show <id>
nt open <id>
nt list tags
nt update <id> <field> <value>
nt agenda
```

Core workflows should remain flagless and composable. The canonical CLI command
and query syntax lives in [cli-syntax-spec.md](cli-syntax-spec.md).

## Command Model

Commands should keep a small, regular grammar:

- Use positional arguments instead of flags for core workflows.
- Use stdin for note input.
- Use stdout for data output.
- Use `$EDITOR` for interactive mutation.
- Use trailing positionals for multiword user text.
- Keep machine-facing commands one-record-per-line.
- Keep mutations to one short lowercase status line.

Avoid broader commands such as `search`, `grep`, `graph`, `browse`,
`agent`, `discuss`, workflow orchestration, or runtime management until real
usage proves they are necessary.

Metadata mutations should go through `nt update <id> <field> <value>` instead of
direct edits to `$HOME/.nt/index.json`. Set-like fields use explicit `+value`
and `-value` operations; single-value fields use a plain value and `-` to clear.

## Query Model

`nt find` uses positional query expressions. All expressions are combined with
`AND`, order does not matter, and search is case-insensitive.

Bare words match searchable metadata or indexed note body terms. The current
implementation keeps a visible index in `$HOME/.nt/index.json`: primary
metadata, derived metadata maps, a metadata `terms` map, and a body term index.
`nt find` uses those indexes for candidate narrowing where possible, intersects
candidate note sets, and then verifies matches before printing. Final output
remains deterministic active-recent order; there is no ranking, fuzzy search, or
semantic search. Markdown file scans are reserved for notes missing from
`body_indexed`; indexed body entries are trusted until `nt rebuild` refreshes
them. Quoted multiword `body:` values match all indexed terms, not an exact
phrase.
Search performance is protected by deterministic structural regression tests,
not wall-clock timing guarantees.

`heading_terms` is indexed for future/internal use only. There is no
`heading:<term>` query field yet.

Unknown query fields should be errors, not bare-word searches. This keeps
filters trustworthy when users or agents mistype field names.

Avoid full boolean syntax, parentheses, scoring, fuzzy search, and regex by
default. If `OR` becomes necessary, add it later with explicit grouping instead
of overloading the v1 `AND` model.

## Search Philosophy

Search/filter speed is a first-class design constraint because the useful unit
of work is getting from a vague memory to the exact note id quickly. `nt` should
prefer narrow, deterministic filters over broad, ranked retrieval.

- Exact metadata filters come first: ids, tags, kinds, statuses, priorities,
  scheduled/due/closed dates, collections, created days, links, and sources.
- Text search should use body term index candidate narrowing before file
  scanning.
- Results should be deterministic, not scored or personalized.
- Machine-facing output should remain stable and one-record-per-line.
- Shell-first workflows should stay the escape hatch for ad hoc inspection.

## Shell-first Human Workflows

A TUI is intentionally deferred and is not part of the current core. The useful
interactive model is:

```text
nt find / nt show / nt open
+ less / fzf / awk / xargs
```

`nt` should keep producing deterministic active-recent output. Shell tools can
provide paging, fuzzy selection, preview, and batching without adding fuzzy
search, interactive prompts, or extra runtime dependencies to `nt`. This keeps
the core usable by both humans and agents.

## Storage Model

Markdown note files are canonical. Notes live in a flat configured notes
directory as `NTYYYYMMDDTHHmmss.md` files. The notes directory should contain
only atomic `.md` note files.

Metadata and text terms live under `$HOME/.nt/index.json` as a visible index.
The index must be written atomically with temp-file-and-rename and should remain
rebuildable where possible. Note bodies must not be stored in the index.

Primary note metadata should stay small:

```text
id
path
created
updated
title
kind
status
priority
scheduled
due
closed
tags
collections
links
sources
```

Derived maps may include:

```text
recent
kinds
statuses
tags
collections
days
backlinks
terms
```

Derived maps must be rebuildable from primary metadata and, where useful, from
CommonMark note bodies.

Current search uses three tiers:

1. Exact id lookup through `notes`.
2. Candidate narrowing through indexed metadata, metadata terms, and the body
   term index.
3. Markdown body reads only for notes missing body index entries.

The `terms` map is a rebuildable inverted index from normalized metadata words
to note ids. The body term index is a rebuildable inverted index from normalized
Markdown body terms to note ids. Indexed body entries are trusted until
`nt rebuild`; stale indexed terms are not file-scanned on every query.

Metadata fields that cannot be derived from CommonMark should be updated through
explicit commands.

## Metadata Model

Use distinct fields instead of overloading tags:

- `kind`: structural form, such as `note`, `todo`, `meeting`, `decision`,
  `source`, `research`, or `project`.
- `status`: agenda state, such as `open`, `waiting`, `done`, or `dropped`.
- `priority`: optional urgency ordered `S`, `A`, `B`, `C`, `D`.
- `scheduled`: optional `YYYY-MM-DD` date when a todo should appear.
- `due`: optional calendar date for a todo, formatted as `YYYY-MM-DD`.
- `closed`: system-managed UTC timestamp for the terminal status transition.
- `collection`: workspace-like group, such as `todos`, `meetings`,
  `projects/nt`, or `research/qemu`.
- `tag`: sparse topic or entity.
- `link`: exact note-to-note relationship stored in JSON metadata.
- `source`: external source reference.

The intended separation is:

- `kind`: what form of note is this?
- `status`: what agenda state is it in?
- `priority`: how urgent is it relative to other todo notes?
- `scheduled`: when should work on it appear in the agenda?
- `due`: when is the todo due?
- `collection`: where does it belong?
- `tag`: what topics or entities are involved?
- `link`: which exact notes are related?

## Kinds And Statuses

Each note should have one `kind`. If no better kind fits, use `note`.

`status` is optional and should stay small. It is primarily for agenda-like
workflows such as todos:

```text
open
waiting
done
dropped
```

`nt agenda` is the read-only action view. It prints notes whose kind is `todo`
and whose status is `open` or `waiting`. Its default sections are Overdue,
Today, Upcoming, Waiting, and Undated. A note appears in exactly one section;
overdue due dates take precedence, then items due today or scheduled on or
before today, then future dated items, waiting items, and undated items.

`nt agenda today`, `week`, `overdue`, `waiting`, and `undated` provide bounded
positional views. Agenda records show id, status, priority, scheduled date, due
date, and title. Priorities sort `S`, `A`, `B`, `C`, `D`, then no priority after
the relevant agenda date.

Changing a todo to `done` or `dropped` records a `closed` UTC timestamp. An
idempotent repeat preserves that timestamp; reopening or clearing status clears
it. Users do not set `closed` directly.

The first agenda must not parse Markdown task syntax or add recurrence, effort
estimates, time tracking, or habits. Those require separate evidence and design
work.

## Collections

Collections are explicit note groups. They are for workspace-like organization,
not semantic keywords and not note-to-note relationships.

Use collections for buckets such as:

```text
todos
meetings
projects/nt
research/qemu
people/alice
```

Collection names should be lowercase and may use `/` for hierarchy-like
organization. This is a naming convention, not nested file storage.

`nt list collections` prints note id, collections, and title. `nt find
collection:<name>` prints notes in one collection using the normal summary
format. `nt update <id> collection +<name>` and its `-<name>` counterpart mutate
visible JSON metadata only; they do not edit the CommonMark note body.

## Links And References

Notes should stay plain CommonMark Markdown. Do not introduce wiki-link syntax,
front matter, or nt-specific note-body markup.

Front matter is allowed only in exported copies. `nt export <path> [id...]`
materializes current JSON metadata into Markdown front matter for
interoperability and archiving, but the active notes directory and
`$HOME/.nt/index.json` remain the canonical storage pair.

Note-to-note links live in JSON metadata. `nt update <id> link +<target-id>` and
its `-<target-id>` counterpart mutate outbound links. `nt list links <id> from`
prints notes linked from the selected note, `nt list links <id> to` prints notes
linking to it, and `nt list links <id>` prints the deduplicated set of direct
relationships.

External source references can live in JSON metadata as `sources`. Markdown links
in the body remain valid CommonMark and may be extracted into `sources` as a
convenience. Saved references should accelerate search, not replace the Markdown
note as the canonical record.

## Tags

Tags should stay sparse and useful. Agents and humans should inspect existing
tags before creating new ones:

```sh
nt list tags
```

Tag rules:

- Prefer existing tags when they accurately describe the note.
- Use one to three tags by default.
- Create a new tag only when no existing tag fits and the concept is likely to
  recur.
- Prefer stable topic or workflow tags over one-off nouns.
- Use lowercase.
- Use kebab-case for multiword tags.
- Avoid plural/singular duplicates when possible.
- Avoid overly broad tags such as `misc`, `notes`, and `important`.

## Agent Use

Agents should filter, inspect, and cite notes through the same visible commands
humans use. `nt` itself must not launch Codex or any other agent, install
skills, generate `AGENTS.md`, implement natural-language retrieval, or maintain
an agent-specific workspace.

Useful agent instructions can live outside `nt` as documentation, copied skill
files, shell snippets, or the host agent's native configuration. The examples in
`docs/examples/agent-skills.md` are documentation only.

Agent-friendly flow:

```sh
nt help
nt list
nt list tags
nt list collections
nt agenda
nt find <expr...>
nt show <id>
```

Agent-driven writes require approval before mutation:

- New notes: produce a CommonMark draft and ask before saving with `nt add`.
- Note edits: produce a proposed replacement or patch, then open `$EDITOR`
  before saving.
- Metadata updates: show planned `nt update <id> <field> <value>` commands
  before running them.

Rejection must leave notes and metadata unchanged.

## Output Model

Output should be plain, stable, grep-friendly, and fast.

- Successful mutations print one short line, such as `saved <id>`.
- Lists use aligned columns: id, date, tags, title.
- `show` prints note identity and metadata before the CommonMark body.
- Prefer lowercase verbs in status output.
- Keep ids visually dominant.
- Keep paths relative when possible.
- Avoid decorative boxes, banners, spinners, and progress bars.
- Use ANSI color only when stdout is a TTY.
- Disable color when stdout is piped, `NO_COLOR` is set, or `TERM=dumb`.
- Machine-facing `list` submodes and `find` must stay stable and
  one-record-per-line.

## Future Extensions

Conservative extensions that fit the project:

- Better tags.
- Project or workspace organization beyond simple collections.
- Research queues.
- Saved source references.
- Import and export.
- Better completion.

Extensions should preserve Markdown notes, visible metadata, explicit commands,
stable output, and the absence of hidden retrieval or background runtime.
