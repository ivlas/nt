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
3. Add simple inline tags when useful, such as `#meeting`, `#decision`,
   `#todo`, `#research`, `#project`, or a concrete topic tag.
4. Pipe the Markdown body to `nt add`.
5. Report the saved note id.

Use this command shape:

```sh
printf '%s\n' '# Title

Concise note body.

#tag' | nt add
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
