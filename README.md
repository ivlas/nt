# nt

> **Status: alpha** `nt` is functional but experimental; expect breaking changes.

`nt` is a local, agent-first application for editable CommonMark notes. Notes
live in SQLite and are available through shell-friendly commands, stdin, and
stdout; capture can also use `$VISUAL`/`$EDITOR`.

## Quick Start

```sh
nt init
printf '%s\n' '# First note' '' 'SQLite is canonical.' | nt add tag:example
nt list tag:example
nt find sqlite
nt show <id>
nt read tag:example
```

`nt add` prints a canonical lowercase UUIDv7 ID. Capture defaults to collection
`inbox`; use `collection:work/nt` to place a note elsewhere.

```sh
printf '%s\n' '# Updated' '' 'Replacement body.' | nt edit <id>
nt move <id> work/nt
nt tag <id> +decision
nt link <id> +<target-id>
nt rm <id>
```

SQLite at `$HOME/.nt/nt.sqlite3` is canonical. There are no filesystem vaults,
configuration files, daemons, background workers, embeddings, model calls in
retrieval, or hidden agent-only commands.

## Architecture

Notes provide editable durable knowledge with collections, tags, and
directional links. Retrieval is deterministic and lexical, with complete
results unless the caller supplies an explicit SQL-backed limit. `nt read`
streams complete filtered notes as JSONL when stdout is redirected.
Committed canonical mutations also receive durable, globally monotonic SQLite
revisions; full-note records expose each live note's latest revision.

External resources, bookmarks, imported documents, and generated reference
summaries can be represented as ordinary CommonMark notes using collections,
tags, and directional links. They do not introduce reserved note kinds, source
metadata, or hidden semantics.

## Documentation

- [Usage](docs/usage.md)
- [CLI reference](docs/cli-reference.md)
- [Architecture](docs/architecture.md)
- [Design](docs/design.md)

## Development

Rust 1.95 is the intentional minimum supported Rust version (MSRV). CI verifies
that version on Linux and current stable Rust on Linux, macOS, and Windows.

Ignored query-plan tests are manual release-mode audits. Run them with
`cargo test --locked --release <audit-name> -- --ignored --exact --nocapture`;
the audit names cover note ID-prefix query plans at representative scale.

## License

MIT; see [LICENSE](./LICENSE).
