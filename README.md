# nt

`nt` is a repo-local, Git-backed command-tree runtime
for developers and agents.

It gives a project one plain Unix-like interface for commands, tools, context,
memory, decisions, artifacts, and traces. Developers and agents use the same
commands, and all durable state is visible as files in the repository.

## Design principles

- Git is required.
- Repo-local only.
- Markdown and plain files are the permanent source of truth.
- No hidden agent-only behavior.
- No hidden retrieval layer, vector database, embeddings, or RAG.
- No hidden state.
- No flags for core flows.
- Tool execution is a core feature.
- Agents mutate workspace state through `nt` commands.
- Every mutating command appends to `.nt/events.jsonl`.
- Fewer dependencies are better.

## Commands

```sh
nt init
nt tree
nt help
nt check
nt log
nt tool list
nt tool run basic
```

## Workspace layout

`nt init` creates runtime state in `.nt/` and visible project knowledge in
`nt/`:

```text
.nt/
  config.toml
  events.jsonl
  tools/
  traces/

nt/
  docs/
  context/
  memory/
  decisions/
  sessions/
  sources/
  artifacts/
```

## Extension model

Tools are plain TOML files in `.nt/tools/`. A minimal manifest looks like:

```toml
name = "basic"
command = "echo hello from nt"
risk = "low"
description = "Example tool"
```

`nt tool run <name>` executes the command, writes a trace to `.nt/traces/`, and
logs an event.
