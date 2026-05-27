# AGENTS.md

## Project

`nt` is a minimal repo-local command layer for developers and agents.

It is a Git-backed command-tree runtime. It exposes project capabilities,
tools, context, memory, decisions, artifacts, and traces through the same
plain command interface for developers and agents.

## Architectural rules

- The binary name is `nt`.
- Git is required.
- Runtime state is repo-local.
- Markdown and plain files remain the source of truth.
- There is no hidden agent-only behavior.
- There is no hidden retrieval layer, vector database, embeddings, or RAG.
- Mutating commands must append an event to `.nt/events.jsonl`.
- Tool execution is a core feature.
- All durable state must be inspectable as plain files.

## Coding style

- Keep modules small.
- Prefer explicit control flow.
- Prefer standard library APIs.
- Avoid clever abstractions.
- Avoid dependencies unless they clearly simplify stable core behavior.
- Keep plain text command output readable for developers and agents.

## Testing expectations

- Run `cargo fmt` before finishing Rust changes.
- Run `cargo test` when behavior changes.
- Run `cargo run -- help` for a basic command smoke test.
- Add focused tests for parsing and command routing.

## Agent constraints

- Agents must not mutate `.nt/` or `nt/` directly when an `nt` command exists.
- Agents must use the same commands available to developers.
- Agents must not add hidden state or hidden retrieval behavior.
- Agents must preserve Markdown and plain files as source of truth.
- Agents must keep workspace mutations visible and logged through `nt`.
