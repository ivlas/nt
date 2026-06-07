use std::fs;
use std::path::PathBuf;

use crate::error::{NtError, Result};
use crate::fs::{atomic_write, nt_home};

pub const REQUIRED_AGENT_SKILLS: &[&str] =
    &["nt-note", "nt-recall", "nt-maintain", "nt-skill-builder"];

pub struct BuiltinSkill {
    pub name: &'static str,
    pub body: &'static str,
}

pub const BUILTIN_SKILLS: &[BuiltinSkill] = &[
    BuiltinSkill {
        name: "nt-note",
        body: NT_NOTE,
    },
    BuiltinSkill {
        name: "nt-recall",
        body: NT_RECALL,
    },
    BuiltinSkill {
        name: "nt-maintain",
        body: NT_MAINTAIN,
    },
    BuiltinSkill {
        name: "nt-skill-builder",
        body: NT_SKILL_BUILDER,
    },
];

pub fn ensure_defaults() -> Result<()> {
    ensure_agents_md()?;

    let dir = skills_dir()?;
    fs::create_dir_all(&dir)?;

    for skill in BUILTIN_SKILLS {
        let skill_dir = dir.join(skill.name);
        let skill_path = skill_dir.join("SKILL.md");

        if skill_path.exists() {
            continue;
        }

        fs::create_dir_all(&skill_dir)?;
        atomic_write(&skill_path, skill.body.as_bytes())?;
    }

    Ok(())
}

pub fn agents_md_path() -> Result<PathBuf> {
    Ok(nt_home()?.join("AGENTS.md"))
}

pub fn installed_agent_skill_bodies() -> Result<Vec<(String, String)>> {
    let mut bodies = Vec::new();

    for name in REQUIRED_AGENT_SKILLS {
        let path = installed_skill_path(name)?;
        if !path.exists() {
            return Err(NtError::Message(format!(
                "missing skill {name}; run `nt init <notes-dir>`"
            )));
        }

        bodies.push((name.to_string(), fs::read_to_string(path)?));
    }

    Ok(bodies)
}

pub fn available_skill_paths() -> Result<Vec<(String, PathBuf)>> {
    let mut paths = Vec::new();

    for skill in BUILTIN_SKILLS {
        paths.push((skill.name.to_string(), installed_skill_path(skill.name)?));
    }

    Ok(paths)
}

fn installed_skill_path(name: &str) -> Result<PathBuf> {
    Ok(skills_dir()?.join(name).join("SKILL.md"))
}

fn skills_dir() -> Result<PathBuf> {
    Ok(nt_home()?.join("skills"))
}

fn ensure_agents_md() -> Result<()> {
    let path = agents_md_path()?;
    if path.exists() {
        return Ok(());
    }

    atomic_write(&path, NT_AGENTS.as_bytes())
}

const NT_AGENTS: &str = r#"# AGENTS.md

## nt workspace

This is the visible agent workspace for `nt`. Use the `nt` CLI as the source of
truth for notes and metadata. Do not rely on hidden memory, embeddings, a vector
store, a daemon, or direct edits to `$HOME/.nt/index.json`.

## Core commands

```sh
nt init <notes-dir>
nt add [metadata...]
nt list
nt find <expr...>
nt show <id>
nt edit <id>
nt discuss <id>
nt discuss <id> <prompt...>
nt rm <id>
nt rebuild
nt ids
nt tags
nt tag <id> <tag>
nt untag <id> <tag>
nt collections
nt collection <name>
nt collect <id> <collection>
nt uncollect <id> <collection>
nt kind <id> <kind>
nt status
nt status <id> <status>
nt link <from-id> <to-id>
nt unlink <from-id> <to-id>
nt links <id> <out|in|self|all>
nt agent <prompt...>
nt config show
nt config agent-output <hidden|format|full>
nt completion <shell>
```

## Find syntax

`nt find` takes trailing positional query expressions. Expressions are combined
with `AND`; order does not matter; search is case-insensitive.

```sh
nt find qemu firecracker
nt find tag:decision qemu
nt find since:2026-05-01 before:2026-06-01 collection:projects/nt
nt find kind:meeting status:open
nt find link:NT20260528T143012
nt find body:'microvm jailer'
nt find not:tag:draft qemu
```

Fields: `id`, `tag`, `title`, `day`, `since`, `before`, `kind`, `status`,
`collection`, `link`, `source`, `body`, and `not:<expr>`. `#tag` is
shorthand for `tag:<tag>`. Unknown fields are errors.

## Usage flow

Retrieve with visible commands first:

```sh
nt list
nt tags
nt collections
nt find <expr...>
nt show <id>
```

When writing notes, draft concise CommonMark Markdown and save through `nt add`.
Use stdin for scripted saves and prefer heredocs for multiline Markdown. When
metadata is known at creation time, pass it to `nt add` with expressions such as
`tag:qemu`, `kind:decision`, `status:open`, `collection:projects/nt`, or
`link:NT20260605T101500`. Repeated fields and comma-separated values are
equivalent for tags, collections, and links.
When changing metadata after creation, use explicit commands such as `nt tag`,
`nt collect`, `nt kind`, `nt status`, and `nt link`. Use `nt rebuild` when the
JSON index appears stale.
"#;

const NT_NOTE: &str = r#"---
name: nt-note
description: >-
  Capture useful research, context, decisions, and durable observations as
  compact Markdown notes with nt.
---

# nt-note

Use `nt` as the visible note system. Capture information with explicit `nt`
commands so humans and other agents can inspect the same record later. Do not
create notes by editing files directly unless an `nt` command is unavailable.

Workflow:

1. Extract the useful context from the user request or current conversation.
2. Write a compact Markdown note with a clear title, the decision or fact, and
   enough context to make it useful later.
3. Choose simple metadata tags when useful, such as `meeting`, `decision`,
   `todo`, `research`, `project`, or a concrete topic tag.
4. Pipe the Markdown body to `nt add`; prefer a heredoc for multiline notes.
   Add creation metadata in the same command when known.
5. Report the saved note id.

Use this command shape:

```sh
cat <<'EOF' | nt add tag:research kind:note
# Title

Concise note body.
EOF
```

Do not store metadata in Markdown front matter. Do not edit
`$HOME/.nt/index.json` directly. Do not rely on hidden memory, embeddings, RAG,
or external retrieval unless the user explicitly asks for that outside `nt`.
"#;

const NT_RECALL: &str = r#"---
name: nt-recall
description: >-
  Use when the user asks what they noted, remembered, saved, decided, discussed,
  or captured earlier with nt.
---

# nt-recall

Retrieve notes through visible `nt` commands before answering.

Workflow:

1. Start broad with `nt list`, `nt tags`, or `nt find <expr...>`.
2. Use `nt show <id>` for each note that may answer the question.
3. Answer from retrieved note contents, not from hidden memory.
4. Cite the note ids that support the answer.
5. If the notes do not contain the answer, say so before using any other source.

Useful commands:

```sh
nt list
nt tags
nt find meeting
nt show NT20260528T143012
```

For time words such as yesterday or last week, inspect note ids and list dates.
Do not use embeddings, hidden retrieval, RAG, or external services unless the
user explicitly asks for outside research.
"#;

const NT_MAINTAIN: &str = r#"---
name: nt-maintain
description: >-
  Use when the nt notebook, note index, tags, ids, completion, or storage needs
  inspection or repair.
---

# nt-maintain

Use visible `nt` commands first to inspect and repair the notebook or index.

Workflow:

1. Use `nt ids`, `nt tags`, and `nt list` to inspect the current index.
2. Use `nt rebuild` when the index looks stale or missing entries.
3. Use `nt show <id>` to verify exact note contents.
4. Only inspect `$HOME/.nt/index.json` when command output is insufficient.
5. Report what changed and cite affected note ids when relevant.

Do not manually edit `$HOME/.nt/index.json` unless no `nt` command can perform
the needed repair. Keep the notes directory flat and limited to atomic
`NTYYYYMMDDTHHmmss.md` files. Do not introduce hidden metadata stores,
embeddings, daemon state, databases, or external retrieval as maintenance
shortcuts.
"#;

const NT_SKILL_BUILDER: &str = r#"---
name: nt-skill-builder
description: >-
  Help the user create or refine custom nt skills for the current workspace.
---

# nt-skill-builder

Use this when the user wants to add or improve an nt skill.

Workflow:

1. Inspect the active nt config and existing skills with visible filesystem or
   `nt config show` output.
2. Draft a compact Markdown skill with a clear name, description, trigger
   guidance, workflow, and constraints.
3. Ask before creating or replacing a skill file.
4. Keep skills agent-agnostic where possible and avoid hidden retrieval,
   background state, or external service requirements.

Custom skills are plain editable Markdown files in the active nt skills
directory. Do not add a separate skill install/list/show command group.
"#;
