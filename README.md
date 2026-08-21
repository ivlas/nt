# nt

> **Status: alpha** `nt` is functional but experimental; expect breaking changes.

`nt` is a local, agent-first knowledge layer for editable CommonMark notes and
external Library evidence. Canonical state lives in one SQLite database.
Humans and agents use the same shell-friendly commands, stdin, stdout, and
`$VISUAL`/`$EDITOR`.

## Quick Start

```sh
nt init
printf '%s\n' '# First note' '' 'SQLite is canonical.' | nt add tag:example
nt list tag:example
nt find sqlite
nt show <id>
printf '%s' 'External evidence' | nt library add https://example.com 'Example'
nt library find evidence
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

`nt` is a modular monolith with an application layer, independent domains, and
shared SQLite infrastructure. Note and Library are independent domains. Domains
own their model, queries, persistence operations, and schema fragments; the
application owns CLI orchestration and explicitly composes the canonical
database schema. Library stores evidence; Notes store synthesis. Future
append-only Memory remains a separate domain requiring its own RFC.

## Documentation

- [Usage](docs/usage.md)
- [CLI reference](docs/cli-reference.md)
- [Architecture](docs/architecture.md)
- [Design](docs/design.md)

## License

MIT; see [LICENSE](./LICENSE).
