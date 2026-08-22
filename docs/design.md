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
| Memory | raw body, sequence, creation timestamp | summaries, pending jobs, raw and summary FTS |

Raw memory is never changed by summary creation, expansion, or invalidation.
Derived memory state can be regenerated from canonical raw history through new
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

SQLite assigns every raw memory a positive, monotonically increasing sequence
number. The sequence is its public identity and deterministic history order.
Database triggers reject raw updates and deletes.

Raw bodies normalize newlines, are non-empty, contain no NUL, and are limited to
1,024 Unicode characters. They have no Markdown title requirement, collection,
tag, link, kind, source, or generic metadata. Summaries have the same character
limit. Each summary node has exactly 16 children, and context output is limited
to 32,768 Unicode characters.

Summarization is delegated to the caller. Appending creates eligible work but
does not wait for it. `nt` does not launch a model, worker, or daemon. Summary
quality depends on the caller; exact raw history remains authoritative.

## Summary Tree

A node is identified by `(level, block)` and written `L<level>:<block>`. For
fanout 16, its inclusive raw range is:

```text
span(level) = 16^(level + 1)
start        = block * span(level) + 1
end          = (block + 1) * span(level)
```

Level zero has 16 raw children. A higher node has 16 summaries from the previous
level. Parent, child, and range relationships are calculated with checked
integer arithmetic rather than stored pointers.

A level-zero job becomes eligible when all 16 raw children exist. A higher job
becomes eligible when all 16 child summaries exist. Children may complete out
of order; the final child creates or repairs the parent job.

Summary creation requires a pending node and all expected children. Resubmitting
identical text is idempotent; different text conflicts. Expansion reveals one
child level. Invalidation removes a summary and all stored ancestors that depend
on it, clears stale jobs, and requeues the selected node when possible. None of
these operations removes raw history.

## Retrieval

Retrieval is deterministic and lexical. User text is tokenized into literal
full-text search (FTS) terms; there is no scoring, fuzzy matching, raw FTS
syntax, embedding search, or automatic model call. SQL applies explicit limits
and stable ordering.

Note `list` and `find` return metadata rows without bodies. They are complete by
default and stream redirected output. Memory `list` and `recall` return exact
raw entries in sequence order.

Memory context uses bounded candidate pools and a complete-output budget:

- Without terms, 60% is offered to recent raw entries and 40% to a canonical
  summary frontier over older history.
- With terms, 40% is offered to lexical raw entries, 30% to recent raw entries,
  and 30% to lexical summaries.
- Recent and lexical SQL candidate queries are limited to 256 rows. Frontier
  construction uses indexed summary availability and fetches at most 256 nodes.
- Unused capacity is offered to bounded recent and broad-history candidates.
  Candidates that do not fit are skipped rather than truncated.

The summary frontier is a non-overlapping set of available nodes representing
older history. It chooses the largest completed summary for each range and
falls back to completed children when possible; ranges without a usable summary
are omitted. The frontier is calculated from sequence bounds and summary
availability, not stored as a table.

Selected raw entries are deduplicated. Selected raw and summary ranges never
overlap, and exact raw evidence wins any overlap conflict regardless of
selection order. Final output is chronological. The 32,768-character limit
includes complete bodies, headers, timestamps, ranges, separators, and newlines.

Fixed candidate bounds keep context predictable but make it non-exhaustive.
Literal search can miss alternate wording, and summaries can omit or misstate
facts. Progressive expansion and raw recall provide exact evidence.

## Storage And Consistency

Only `nt init` creates storage. The database uses application ID `0x4e544e54`
(`NTNT`) and clean-sheet schema version `2`. This alpha policy rejects
incompatible databases instead of migrating them in place. Every mutation is
transactional, including its relationship and full-text index changes.

Schema ownership, connection lifecycles, and transaction boundaries are
described in [Architecture](architecture.md). Documentation intentionally does
not duplicate schema SQL.
