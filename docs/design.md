# nt Design

## Project idea

`nt` is a small CLI-native note organizer and research workspace for humans and
agents. The primary design target is coding agents that need visible,
deterministic knowledge operations: capture, browse, recall, maintain, and
extend Markdown notes through explicit commands.

Humans use the same interface. There is no separate agent memory layer.

## Non-goals

`nt` is not an agent framework, RAG system, vector database, daemon, server,
browser runtime, microVM orchestrator, workflow engine, or Hermes replacement.
It should not grow hidden retrieval, embeddings, background state, or framework
complexity as core behavior.

## Core workflows

The core loop is:

```text
capture -> organize -> retrieve -> inspect -> revise -> rebuild
```

The loop maps to small commands:

```sh
nt add
nt list
nt find <query...>
nt show <id>
nt edit <id>
nt tags
nt rebuild
nt agent <prompt...>
```

Core workflows should remain flagless and composable with stdin, stdout,
`$EDITOR`, and normal shell tools.

## Command model

Commands should keep a small, regular grammar:

- Use positional arguments instead of flags for core workflows.
- Use stdin for note input.
- Use stdout for data output.
- Use `$EDITOR` for interactive mutation.
- Use trailing positionals for multiword user text.
- Keep machine-facing commands one-record-per-line.
- Keep mutations to one short lowercase status line.

Recommended command surface:

```sh
nt init <notes-dir>
nt add
nt list
nt find <query...>
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

`nt links <id>` and `nt backlinks <id>` should be added only when note links are
stored in metadata. They should print note ids one per line.

Metadata mutations should go through explicit commands instead of direct edits
to `$HOME/.nt/index.json`.

`nt edit <id>` opens the CommonMark note in `$EDITOR`. `nt discuss <id>` opens a
Codex session seeded with `nt show <id>` output so the user can continue a
discussion from a specific note. With trailing prompt text, it should use that as
the initial user request:

```sh
nt discuss NT20260605T101500
nt discuss NT20260605T101500 continue from this decision and compare the risks
```

Avoid adding broader commands such as `search`, `grep`, `graph`, `open`,
`browse`, workflow orchestration, or runtime management until real usage proves
they are necessary.

## Query language

`nt find` should use a small token-based query language. Space means `AND`.
Search is case-insensitive. Bare words search common indexed fields first and
fall back to streaming Markdown bodies only when needed.

Examples:

```sh
nt find qemu
nt find qemu firecracker
nt find tag:decision qemu
nt find title:firecracker tag:vm
nt find day:2026-05-28
nt find after:2026-05-01 before:2026-06-01 qemu
nt find kind:meeting
nt find status:open
nt find collection:meetings
nt find link:NT20260528T143012
nt find ref:firecracker
nt find text:"microvm jailer"
nt find not:tag:draft qemu
```

Initial fields:

- `id:<id>`: exact or prefix note id match.
- `tag:<tag>`: notes with a tag.
- `#tag`: shorthand for `tag:<tag>`.
- `title:<term>`: title contains term.
- `day:<date>`: notes created on `YYYY-MM-DD`.
- `after:<date>`: notes created on or after `YYYY-MM-DD`.
- `before:<date>`: notes created before `YYYY-MM-DD`.
- `kind:<kind>`: notes of a structural kind.
- `status:<status>`: notes with an agenda status.
- `collection:<name>`: notes in a collection.
- `link:<id>`: notes that link to an id.
- `ref:<term>`: saved source/reference contains term.
- `text:<term>`: force note body search.
- `not:<expr>`: exclude a simple expression.

Avoid full boolean syntax, parentheses, scoring, fuzzy search, and regex by
default. If `OR` becomes necessary, prefer an explicit token form later, such as
`or:qemu or:firecracker tag:vm`.

## Agent model

Agents should retrieve and cite notes through visible commands. `nt agent` is a
thin Codex launcher that provides editable nt skills from the active vault
and shells out to `codex exec` for one-shot agent work. `nt discuss <id>` is the
interactive counterpart: it should open Codex with a specific note and its
visible metadata as context. `nt` itself does not implement natural-language
retrieval.

Default nt skills should be created automatically during `nt init`; there should
not be a separate `nt skill install`, `nt skill list`, or `nt skill show`
command group. `nt config show` should print the active config, active vault,
agent workspace, and available skill names/paths.

Preferred vault layout:

```text
<vault>/
  metadata.json
  notes/
    NT20260605T101500.md
  skills/
    nt-note.md
    nt-recall.md
    nt-maintain.md
    nt-skill-builder.md
  workspace/
```

`workspace/` is the agent working directory. Skills are durable vault
instructions, so they live beside `workspace/`, not inside it. When `nt agent`
or `nt discuss` runs, `nt` should set the process cwd to `<vault>/workspace`
and pass the active skills as context.

When answering from notes, agents should inspect exact note bodies with
`nt show <id>` and cite the supporting note ids.

Agent-driven note creation or metadata mutation should require approval before
writing. Retrieval can run directly through visible commands, but changes should
be explicit:

- For a new note, Codex should produce a CommonMark draft; `nt` should present
  it for approval before saving with `nt add`.
- For note edits, Codex should produce a proposed replacement or patch; `nt`
  should open the result in `$EDITOR` before saving.
- For metadata updates, Codex should show the planned commands, such as
  `nt collect`, `nt link`, `nt kind`, or `nt status`, before running them.

Approval should result in normal mutation output such as `saved <id>` or
`linked <from-id> <to-id>`. Rejection should leave notes and metadata unchanged.

Default skills:

- `nt-note`: capture useful research, context, decisions, and observations.
- `nt-recall`: retrieve with visible nt commands and cite note ids.
- `nt-maintain`: inspect and repair metadata with visible nt commands.
- `nt-skill-builder`: help the user create or refine custom nt skills for their
  vault.

Custom skills are plain editable Markdown files in `<vault>/skills`. Agents
should use `nt-skill-builder` when the user asks to create a new custom skill or
adapt an existing one.

## Storage model

Markdown files are canonical. Notes live in a flat notes directory as
`NTYYYYMMDDTHHmmss.md` files. Metadata lives under `$HOME/.nt/index.json` as
visible JSON and must remain rebuildable where possible.

The index stores small metadata such as ids, paths, timestamps, titles, kinds,
statuses, tags, collections, links, saved source references, recent ids, kind
maps, status maps, tag maps, collection maps, day maps, link maps, and term
maps. It does not store note bodies.

Recommended shape:

```json
{
  "version": 2,
  "notes": {
    "NT20260528T143012": {
      "id": "NT20260528T143012",
      "path": "/path/to/notes/NT20260528T143012.md",
      "created": "2026-05-28T14:30:12Z",
      "updated": "2026-05-28T14:30:12Z",
      "title": "Firecracker vs QEMU decision",
      "kind": "decision",
      "status": null,
      "tags": ["vm", "firecracker", "qemu", "decision"],
      "collections": ["projects/nt", "research/vm"],
      "links": ["NT20260520T101500"],
      "refs": ["https://firecracker-microvm.github.io/"],
      "words": ["firecracker", "qemu", "microvm", "jailer"]
    }
  },
  "recent": ["NT20260528T143012"],
  "kinds": {
    "decision": ["NT20260528T143012"]
  },
  "statuses": {},
  "tags": {
    "qemu": ["NT20260528T143012"]
  },
  "collections": {
    "projects/nt": ["NT20260528T143012"],
    "research/vm": ["NT20260528T143012"]
  },
  "days": {
    "2026-05-28": ["NT20260528T143012"]
  },
  "backlinks": {
    "NT20260520T101500": ["NT20260528T143012"]
  },
  "terms": {
    "qemu": ["NT20260528T143012"]
  }
}
```

Primary note metadata should stay small. Derived maps should be rebuildable from
primary metadata and, where useful, from CommonMark note bodies.

Search should use three tiers:

1. Exact id lookup through `notes`.
2. Indexed metadata lookup through kinds, statuses, tags, collections, days,
   links, refs, titles, and terms.
3. Streaming body search as fallback.

The `terms` map is a rebuildable inverted index from normalized words to note
ids. Start with words from titles, kinds, statuses, tags, collections, note ids,
links, headings, references, and possibly the first paragraph. Indexing every
body word can wait until real vault size requires it.

Metadata fields that cannot be derived from CommonMark should be updated through
commands such as `nt collect`, `nt kind`, `nt status`, and `nt link`, not by
manual index editing.

## Kinds and agenda status

`kind` is a low-cardinality structural field. It describes the form of the note,
not its topic and not where it belongs.

Suggested initial kinds:

```text
note
todo
meeting
decision
source
research
project
```

Each note should have one kind. If no better kind fits, use `note`.

`status` is for agenda-like workflows, especially `kind:todo` notes. It should
stay small and optional:

```text
open
waiting
done
dropped
```

Commands:

```sh
nt kind NT20260605T101500 meeting
nt status
nt status NT20260605T101500 open
nt status NT20260605T101500 done
```

Bare `nt status` should print actionable notes in stable order, primarily open
and waiting todos, using the normal summary format. With an id and status,
`nt status <id> <status>` mutates visible JSON metadata. This keeps agenda-like
review under the same command as agenda status changes.

`nt status` may later include dated meetings or scheduled items if date metadata
is added, but it should not become a workflow engine.

The intended separation is:

- `kind`: what form of note is this?
- `status`: what agenda state is it in?
- `collection`: where does it belong?
- `tag`: what topics or entities are involved?
- `link`: which exact notes are related?

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

Collections answer "where does this note belong?" Tags answer "what is this
about?" Links answer "which specific note is related?"

A note may belong to zero or more collections:

```json
"collections": ["meetings", "projects/nt"]
```

The index should maintain a derived collection map for fast lookup:

```json
"collections": {
  "meetings": ["NT20260605T101500"],
  "projects/nt": ["NT20260605T101500"]
}
```

Collection commands should stay positional and flagless:

```sh
nt collections
nt collection meetings
nt collect NT20260605T101500 meetings
nt uncollect NT20260605T101500 meetings
```

`nt collections` prints known collection names, one per line. `nt collection
<name>` prints notes in that collection using the normal summary format.
`nt collect` and `nt uncollect` mutate visible JSON metadata only; they do not
edit the CommonMark note body.

Search should support collections directly:

```sh
nt find collection:meetings
nt find collection:todos tag:urgent
nt find collection:projects/nt qemu
```

Collection names should be lowercase and may use `/` for hierarchy-like
organization. This is a naming convention, not nested storage.

## Links and references

Notes should stay plain CommonMark Markdown. Do not introduce wiki-link syntax,
front matter, or nt-specific markup into note bodies.

Note links should live in visible JSON metadata. A note may have outbound links
to other note ids:

```json
"links": ["NT20260520T101500"]
```

The index should also maintain a derived backlinks map:

```json
"backlinks": {
  "NT20260520T101500": ["NT20260528T143012"]
}
```

`nt show <id>` should print note identity and metadata before the CommonMark
body so the body boundary remains clear:

```text
NT20260605T101500  Firecracker vs QEMU decision
path notes/NT20260605T101500.md
kind decision
status -
collections projects/nt,research/vm
tags vm,firecracker,qemu
links NT20260520T101500
backlinks NT20260601T090000

# Firecracker vs QEMU decision

Use Firecracker for constrained microVM isolation.
```

`nt link <from-id> <to-id>` and `nt unlink <from-id> <to-id>` should mutate
visible JSON metadata. `nt links <id>` and `nt backlinks <id>` should print note
ids one per line for scripts and agents.

External source references can also live in JSON metadata as `refs`. Markdown
links in the body remain valid CommonMark and may be extracted into `refs` as a
convenience, but note-to-note links should not require special Markdown syntax.
Saved references should accelerate search, not replace the Markdown note as the
canonical record.

## Tag model

Tags should stay sparse and useful. Agents and humans should inspect the
existing vocabulary before creating new tags:

```sh
nt tags
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

The `nt-note` skill should enforce this during note creation. A separate tag
skill is unnecessary until tag cleanup becomes a distinct workflow.

## Output model

Output should be plain, stable, grep-friendly, and fast. Machine-facing commands
such as `nt ids`, `nt find`, and `nt tags` should stay one-record-per-line.
Successful mutations should print one short lowercase status line.

ANSI color is optional and must only be used when stdout is a TTY.

## Future extensions

Conservative extensions that fit the project:

- Better tags.
- Note links.
- Backlinks.
- Project or vault organization beyond simple collections.
- Research queues.
- Saved source references.
- Import and export.
- Better completion.

These should preserve Markdown notes, visible metadata, explicit commands,
stable output, and the absence of hidden retrieval or background runtime.
