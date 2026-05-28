# nt Usage Guide

`nt` is a small note-taking CLI. Notes are plain Markdown files, while metadata
lives in `$HOME/.nt/index.json`.

## Setup

Create or select a notes directory:

```sh
nt init notes
```

This creates the notes directory and configures it as the active notebook.

## Add Notes

Add a note from stdin:

```sh
printf '%s\n' '# Storage decision

Keep note metadata outside Markdown.

#decision #nt' | nt add
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

`nt rebuild` scans `NTYYYYMMDDTHHmmss.md` files and recreates derived metadata.

## Shell Completion

Generate completion scripts with `clap_complete`:

```sh
nt completion zsh
nt completion bash
nt completion fish
```

Note id completion should be backed by:

```sh
nt ids
```

## Codex Agent Flow

Install nt skills:

```sh
nt skill install
```

This creates editable skills under:

```text
$HOME/.nt/skills
```

List or inspect them:

```sh
nt skill list
nt skill show nt-note
nt skill show nt-recall
nt skill show nt-maintain
```

Use `nt agent <prompt...>` to launch Codex with those skills:

```sh
nt agent note this decision about metadata outside markdown
nt agent what did I note about storage?
```

`nt agent` shells out to `codex exec`. It does not implement natural language
retrieval itself.

Configure Codex output:

```sh
nt config agent-output hidden
nt config agent-output format
nt config agent-output full
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
