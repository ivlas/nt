# nt Usage Guide

`nt` is a small note organizer and CLI research workspace for humans and
agents. Notes are plain Markdown files, while metadata lives in
`$HOME/.nt/index.json`.

See [cli-syntax-spec.md](cli-syntax-spec.md) for the compact CLI command and
query syntax contract.

## Setup

Create or select a notes directory:

```sh
nt init notes
```

This creates the notes directory and configures it as active.

## Add Notes

Add a multiline Markdown note from stdin:

```sh
cat <<'EOF' | nt add
# Storage decision

Keep note metadata outside Markdown.
EOF
```

Attach visible metadata while creating the note:

```sh
cat <<'EOF' | nt add tag:storage kind:decision status:open collection:projects/nt
# Storage decision

Keep note metadata outside Markdown.
EOF
```

Repeated metadata fields and comma-separated values are equivalent:

```sh
nt add tag:qemu,firecracker tag:research kind:decision
nt add tag:qemu,firecracker,research kind:decision
nt add tag:qemu tag:firecracker tag:research kind:decision
```

Link the new note to one or more existing notes during creation:

```sh
cat <<'EOF' | nt add link:NT20260605T101500,NT20260605T103000 tag:followup
# Follow-up

Connect this note to two earlier notes.
EOF
```

If stdin is a terminal, `nt add` opens `$EDITOR`:

```sh
nt add
```

Successful writes print the note id:

```text
saved NT20260528T143012
```

The note is stored as:

```text
notes/NT20260528T143012.md
```

## Find And Read

List recent notes:

```sh
nt list
```

Search metadata and note bodies:

```sh
nt find storage
nt find since:2026-05-01 before:2026-06-01 tag:decision collection:projects/nt
```

Show one exact note:

```sh
nt show NT20260528T143012
```

Print ids for completion, scripts, or agents:

```sh
nt ids
```

Print known tags:

```sh
nt tags
```

## Organize Metadata

List and inspect collections:

```sh
nt collections
nt collection projects/nt
```

Update visible JSON metadata with explicit commands:

```sh
nt collect NT20260528T143012 projects/nt
nt uncollect NT20260528T143012 projects/nt
nt tag NT20260528T143012 storage
nt untag NT20260528T143012 storage
nt kind NT20260528T143012 decision
nt status NT20260528T143012 open
nt link NT20260528T143012 NT20260527T120000
nt unlink NT20260528T143012 NT20260527T120000
```

Print note links for scripts and agents:

```sh
nt links NT20260528T143012
nt backlinks NT20260527T120000
```

Show actionable open and waiting notes:

```sh
nt status
```

## Agent Research Flow

Agents use the same visible commands as humans. Start with stable, cheap
retrieval:

```sh
nt find qemu
nt show NT20260528T143012
```

Then answer from the retrieved Markdown and cite the supporting note ids. For
agent-assisted work, launch Codex through `nt agent`:

```sh
nt agent "what did I previously decide about Firecracker vs QEMU?"
nt agent "research this topic and save a compact note"
```

`nt agent` shells out to `codex exec` and gives Codex visible nt skills from the
active workspace. `nt` itself does not implement hidden natural-language
retrieval, embeddings, RAG, or external memory. If an answer depends on notes,
the agent should retrieve them with commands such as `nt find`, `nt list`,
`nt tags`, and `nt show`.

## Edit And Remove

Edit a note with `$EDITOR`:

```sh
nt edit NT20260528T143012
```

Remove a note:

```sh
nt rm NT20260528T143012
```

## Rebuild Index

If the metadata index is stale, rebuild it from the active notes directory:

```sh
nt rebuild
```

`nt rebuild` scans `NTYYYYMMDDTHHmmss.md` files and recreates derived metadata,
including cheap term indexes from headings, Markdown links, and the first
paragraph. For known note ids, it preserves explicit metadata that cannot be
derived from Markdown, such as kind, status, tags, collections, links, and sources.

## Shell Completion

Generate completion scripts with `clap_complete`:

```sh
nt completion zsh
nt completion bash
```

Note id completion should be backed by:

```sh
nt ids
```

## Codex Agent

Default `AGENTS.md` and nt skills are created by `nt init` in `$HOME/.nt`.
Show the active config, agent workspace, `AGENTS.md`, and available skills:

```sh
nt config show
```

Use `nt agent <prompt...>` to launch Codex from that agent workspace with those
skills:

```sh
nt agent note this decision about metadata outside markdown
nt agent what did I note about storage?
```

`nt agent` is a launcher, not an agent framework or retrieval layer.

Configure Codex output:

```sh
nt config agent-output hidden
nt config agent-output format
nt config agent-output full
```

This writes `$HOME/.nt/config.toml`:

```toml
[agent]
backend = "codex"
output = "format"
```

Modes:

- `hidden`: show status only.
- `format`: show the extracted Codex answer.
- `full`: show the full Codex output.

Show current config:

```sh
nt config show
```

## Unix Composition

Use normal shell tools around stable one-record-per-line output:

```sh
nt ids | head
nt find meeting | awk '{print $1}'
nt tags | sort
```

Agents should prefer the same visible flow:

```sh
nt find meeting
nt show NT20260528T143012
```

No embeddings, daemon, database, vector store, or hidden retrieval is required.
