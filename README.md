# nt

> **Status: alpha** `nt` is functional but experimental; expect breaking changes.

`nt` is a local, agent-first application for editable CommonMark notes and
immutable persistent memory. Canonical notes and raw experience live in one
SQLite database as separate first-class models. Humans and agents use the same
shell-friendly commands, stdin, and stdout; note capture can also use
`$VISUAL`/`$EDITOR`.

## Quick Start

```sh
nt init
printf '%s\n' '# First note' '' 'SQLite is canonical.' | nt add tag:example
nt list tag:example
nt find sqlite
nt show <id>
printf '%s\n' 'Deployment switched to blue-green.' | nt memory add
nt memory recall deployment
nt memory context deployment
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

`nt memory add` prints a monotonically increasing sequence number. Raw memories
are canonical and immutable. The calling agent can use `nt memory pending` and
`nt memory summarize` to build the derived 16-way summary pyramid, then recover
exact history progressively with `nt memory expand`.

SQLite at `$HOME/.nt/nt.sqlite3` is canonical. There are no filesystem vaults,
configuration files, daemons, background workers, embeddings, model calls in
retrieval, or hidden agent-only commands.

## Architecture

Notes and memory are separate concrete models. Notes provide editable durable
knowledge with collections, tags, and directional links. Memory provides this
fixed architecture:

```text
immutable raw experience -> 16-way summary pyramid -> indexed retrieval
-> 32 KiB context compiler -> progressive expansion -> exact original history
```

Storage size and context size are independent. Raw history remains in SQLite;
the context compiler selects at most 32,768 Unicode characters of complete
memory content for one invocation. Summaries, summary jobs, and FTS indexes are
derived and rebuildable from immutable raw memory. Summarization is explicit
work performed by the calling agent, not a daemon or automatic model call.

External resources, bookmarks, imported documents, and generated reference
summaries can be represented as ordinary CommonMark notes using collections,
tags, and directional links. They do not introduce reserved note kinds, source
metadata, or hidden semantics.

## Documentation

- [Usage](docs/usage.md)
- [CLI reference](docs/cli-reference.md)
- [Architecture](docs/architecture.md)
- [Design](docs/design.md)

## License

MIT; see [LICENSE](./LICENSE).
