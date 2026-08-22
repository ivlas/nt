# nt

> **Status: alpha** `nt` is functional but experimental; expect breaking changes.

`nt` is a local, agent-first note application for editable CommonMark,
deterministic metadata, and lexical retrieval. Canonical notes live in one
SQLite database. Humans and agents use the same shell-friendly commands, stdin,
stdout, and `$VISUAL`/`$EDITOR`.

## Quick Start

```sh
nt init
printf '%s\n' '# First note' '' 'SQLite is canonical.' | nt add tag:example
nt list tag:example
nt find sqlite
nt show <id>
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
configuration files, daemons, embeddings, or hidden agent-only commands.

## Architecture

`nt` is deliberately single-purpose: it takes, organizes, links, and retrieves
notes. CLI adapters feed command orchestration, commands use the concrete note
model and repository, and storage configures and validates SQLite. The note
module owns note values, queries, persistence operations, and note schema SQL;
the application owns one fixed, flat schema manifest.

External resources, bookmarks, imported documents, and generated summaries can
be represented as ordinary CommonMark notes using collections, tags, and
directional links. They do not introduce reserved note kinds, source metadata,
or hidden semantics.

## Documentation

- [Usage](docs/usage.md)
- [CLI reference](docs/cli-reference.md)
- [Architecture](docs/architecture.md)
- [Design](docs/design.md)

## License

MIT; see [LICENSE](./LICENSE).
