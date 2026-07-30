# nt

> **Status: alpha** `nt` is functional but experimental; expect breaking changes.

`nt` is a local, agent-first knowledge and memory layer optimized for fast
capture, deterministic retrieval, low-cost context construction, and full local
ownership. Notes and metadata live in one portable SQLite database. Humans and
agents use the same shell-friendly commands, stdin, stdout, and `$EDITOR`.

## Quick Start

```sh
nt init personal
printf '%s\n' '# First Note' '' 'body text' | nt note tag:example
nt find example
```

`nt note` prints a canonical UUIDv7 id like
`018fbe0a-6c00-7000-8000-000000000001`.

```sh
nt show <id>
nt open <id>
nt list
nt agenda
```

Collections are logical `<vault>/<collection>` namespaces:

```sh
nt init work
printf '%s\n' '# Shared' | nt note home:personal/rust collection:work/project_a
```

## Documentation

- [docs/usage.md](docs/usage.md)
- [docs/cli-reference.md](docs/cli-reference.md)
- [docs/design.md](docs/design.md)

## License

MIT; see [LICENSE](./LICENSE).
