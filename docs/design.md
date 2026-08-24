# nt Design

This document records product invariants and retrieval policy. The public
command contract is in [CLI Reference](cli-reference.md); code ownership is in
[Architecture](architecture.md).

## Product Model

`nt` is a local, agent-first application with two separate models:

- **Notes** hold editable durable knowledge as CommonMark documents with one
  collection, optional tags, and directional links.
- **Memory** holds immutable durable experience as one ordered sequence, with
  derived summaries for bounded retrieval.

SQLite in the resolved home directory is canonical. There is no canonical
Markdown vault, JSON index, or rebuild-from-files workflow. Notes and memory
have separate schemas and concrete repositories; they are not variants of a
generic entity.

There are no note kinds, todos, collection entities, additional memberships,
sources, generic metadata, reserved tags, or hidden agent-only semantics.
Bookmarks, imports, external resources, and generated reference documents are
ordinary notes. Persistent experience belongs to memory, not a special note.

## Canonical And Derived State

| Model | Canonical state | Derived state |
| --- | --- | --- |
| Notes | body, collection, tags, links, timestamps, body version | title, note FTS |
| Memory | raw body, sequence, creation timestamp | binary range summaries |

Raw memory is never changed by summary creation, zooming, or forgetting.
Derived memory state can be recreated from canonical raw history through new
caller-produced summaries, but the text is not guaranteed to match an earlier
summary. The CLI does not provide a whole-database rebuild command.

## Notes

Public note IDs are canonical lowercase UUIDv7 values. A canonical body is
non-empty CommonMark with CRLF and CR normalized to LF. Its first line begins
with `# ` and contains a non-whitespace title. The trimmed remainder of that
line is stored as the title; other body content is preserved.

Every note has exactly one collection, defaulting to `inbox`. A collection is a
lowercase path with `/`-separated segments; tags use the same lowercase
characters without `/`. Tags and links are sets. Links are explicit,
directional, and cannot target the source note itself.

Body edits use an expected body version so two editor sessions cannot silently
overwrite each other. Metadata changes do not conflict with an open editor.
No-op set changes preserve `updated`; real metadata changes update it. UTC
timestamps have one-second resolution and are not a monotonic mutation order.

Multi-note deletion is atomic. Deleting a target removes incoming edges and
updates surviving sources because their outgoing-link sets changed. Deleting a
source does not update its targets.

## Memory

Every raw memory receives the next contiguous sequence number beginning at zero.
The sequence is its public identity and deterministic history order. Database
triggers reject raw updates and deletes.

Raw bodies are non-empty single lines, contain no NUL, and are limited to 512
Unicode characters. They have no Markdown title requirement, collection, tag,
link, kind, source, or generic metadata. Summaries follow the same body rules.

Summary creation is delegated to the caller. Raw insertion does not wait for it.
`nt` does not launch a model, worker, or daemon. Summary quality depends on the
caller; exact raw history remains authoritative.

## Summary Tree

A summary is identified internally by an aligned binary half-open range
`[lo, hi)`. Its size is a power of two of at least two, and `lo` is a multiple
of that size. The CLI renders the upper bound inclusively, so `[0, 2)` is `0-1`
and `[0, 8)` is `0-7`.

```text
size = hi - lo
mid  = lo + size / 2
children = [lo, mid), [mid, hi)
```

A size-two summary has two raw children. A larger summary has two direct summary
children. Parent, child, and range relationships are calculated with checked
integer arithmetic rather than stored pointers.

`nap` scans by increasing range size and then increasing start, returning the
smallest buildable missing summary. A pair is buildable from two raw memories; a
larger range is buildable from its two stored child summaries. There is no
persisted work queue.

Summary creation requires both direct children. Resubmitting identical text is
idempotent; different text conflicts. `zoom` reveals one child level. `forget`
removes the selected summary and all stored ancestors that depend on it, but no
descendants or raw history.

## Retrieval

Note retrieval is deterministic and lexical. Note text is tokenized into literal
FTS terms; there is no scoring, fuzzy matching, raw FTS syntax, embedding search,
or automatic model call. Note `list` and `find` return metadata rows without
bodies, are complete by default, and stream redirected output.

Memory retrieval is direct and separate. `recall` scans canonical raw bodies for
one case-sensitive literal substring and emits matches chronologically. It has
no FTS, tokenization, stemming, ranking, fuzzy matching, or semantic search.

`wake` constructs a deterministic chronological age-decaying dyadic cover. The
compile-time `WAKE_ENTRIES = 128` entry count is its only bound. Histories at or
below the bound are all raw. Larger histories start from their canonical aligned
binary cover and repeatedly split the newest splittable range until the bound is
filled, making older coverage coarser and recent coverage more precise. Every
selected summary must already exist; missing derived state is an error rather
than an implicit model call or fallback.

## Storage And Consistency

Only `nt init` creates storage. The database uses application ID `0x4e544e54`
(`NTNT`) and clean-sheet schema version `4`. There is no migration system;
incompatible databases are rejected. Every mutation is transactional, including
its relationship and note full-text index changes.

Schema ownership, connection lifecycles, and transaction boundaries are
described in [Architecture](architecture.md). Documentation intentionally does
not duplicate schema SQL.
