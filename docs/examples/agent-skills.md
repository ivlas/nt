# Agent Skill Examples

These are documentation examples only. `nt init` does not install them, and
`nt` does not run agents. Copy or adapt the examples into the skill/config
format used by your agent.

## nt-note

Use when the user asks to remember, save, capture, or record something in `nt`.

````markdown
---
name: nt-note
description: Capture useful context as compact Markdown notes with nt.
---

# nt-note

Use `nt` as the visible note system. Do not edit note files or
`$HOME/.nt/index.json` directly.

Workflow:

1. Draft a concise CommonMark note with a clear title.
2. Run `nt tags` and `nt collections` before choosing metadata.
3. Prefer one to three existing tags.
4. Ask before mutation when the user has not already approved saving.
5. Save with `nt add [metadata...]`.
6. Report the saved note id.

Example:

```sh
cat <<'EOF' | nt add tag:research kind:note collection:projects/nt
# Title

Concise note body.
EOF
```
````

## nt-recall

Use when the user asks what they noted, saved, decided, discussed, or captured
earlier. Optimize for `time-to-knowledge`: get from vague memory to exact note
ids and the note content behind them quickly.

````markdown
---
name: nt-recall
description: Retrieve notes with visible nt commands and cite note ids.
---

# nt-recall

Retrieve through visible commands only. Prefer exact metadata filters and
indexed text search before file scanning.

Workflow:

1. Start with cheap indexes: `nt list`, `nt tags`, `nt collections`, or
   `nt ids`.
2. Use exact metadata filters and `nt find <expr...>` for candidate notes.
3. Use `nt show <id>` before relying on a note.
4. Answer from shown note content and cite supporting note ids.
5. Do not rely on hidden memory, embeddings, or direct index edits.

Examples:

```sh
nt find tag:decision qemu
nt find since:2026-05-01 before:2026-06-01 collection:projects/nt
nt show NT20260528T143012
```
````

## nt-maintain

Use when the user asks to clean up note metadata or inspect the workspace.

````markdown
---
name: nt-maintain
description: Inspect and maintain nt metadata with explicit commands.
---

# nt-maintain

Use explicit metadata commands. Do not edit `$HOME/.nt/index.json` directly.

Workflow:

1. Inspect current state with `nt config show`, `nt tags`, `nt collections`,
   `nt status`, and targeted `nt find` commands.
2. Propose exact commands before mutating metadata.
3. Use `nt tag`, `nt untag`, `nt collect`, `nt uncollect`, `nt kind`,
   `nt status`, `nt link`, and `nt unlink`.
4. Verify with `nt show <id>` or the relevant list command.

Examples:

```sh
nt tag NT20260528T143012 storage
nt collect NT20260528T143012 projects/nt
nt status NT20260528T143012 done
nt link NT20260528T143012 NT20260527T120000
```
````

## nt-skill-builder

Use when the user wants to create or refine their own agent skill for `nt`.

````markdown
---
name: nt-skill-builder
description: Help write agent-specific skills that use nt as a CLI tool.
---

# nt-skill-builder

`nt` does not install or run skills. Help the user create instructions for
their chosen agent system.

Workflow:

1. Ask which agent system or skill format they use if it is not clear.
2. Base the skill on `nt help` and `docs/cli-syntax-spec.md`.
3. Keep the skill agent-agnostic where possible: visible commands, explicit
   note ids, no hidden retrieval.
4. Include mutation safety: draft first, ask before saving when needed.
5. Tell the user where to place the skill according to their agent system.
````
