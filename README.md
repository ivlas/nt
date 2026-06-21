# nt

> **Status: alpha** `nt` is functional but experimental, built around my note-taking workflow and agent-based knowledge management; expect rough edges, bugs, and breaking changes.

`nt` is a small CLI-native note organizer: canonical CommonMark notes in a flat
vault, visible JSON metadata, deterministic search, and shell-friendly commands.
Humans and agents share the same Unix interface — stdin, stdout, `$EDITOR`,
one-record-per-line output, no hidden memory layer.


## Quick Start

```sh
nt init notes
printf '%s\n' '# First Note' '' 'body text' | nt add tag:example
nt find example          # prints NT20260616T101500-style ids
```

`nt add` prints a note id like `NT20260616T101500`.

```sh
nt show <id>
nt open <id>             # edit in $EDITOR
nt list                  # id title kind status due tag
nt agenda                # open/waiting todos
nt rebuild               # after out-of-band file edits or deletes
```

## Documentation

- [docs/usage.md](docs/usage.md) — task-oriented workflows and shell recipes
- [docs/cli-reference.md](docs/cli-reference.md) — complete command, query,
  value, and output contract
- [docs/design.md](docs/design.md) — architecture and decisions

## License

MIT — see [LICENSE](./LICENSE)
