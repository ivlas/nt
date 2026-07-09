# nt

> **Status: alpha** `nt` is functional but experimental, built around a user-owned note-taking workflow; expect rough edges, bugs, and breaking changes.

`nt` is a small CLI-native note organizer: canonical CommonMark notes in a flat
vault, visible JSON metadata, deterministic search, and shell-friendly commands.
The user owns the notes and directs every mutation. An agent may use the same
Unix interface — stdin, stdout, `$EDITOR`, and one-record-per-line output — only
when the user asks it to do so; there is no hidden memory layer or autonomous
note-taking behavior.


## Quick Start

```sh
nt init notes
printf '%s\n' '# First Note' '' 'body text' | nt note tag:example
nt find example          # prints NT20260616T101500-style ids
```

`nt note` prints a note id like `NT20260616T101500`.

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
