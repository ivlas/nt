use std::process::{Command as ProcessCommand, Stdio};

use crate::config::{AgentOutputMode, Config};
use crate::error::{NtError, Result};
use crate::fs::nt_home;
use crate::skills;

pub fn run(prompt: &[String]) -> Result<()> {
    let config = Config::load()?;
    let user_prompt = prompt.join(" ");
    let skill_bodies = skills::installed_agent_skill_bodies()?;
    let prepared_prompt = build_agent_prompt(&skill_bodies, &user_prompt);

    run_codex(&config, &prepared_prompt)
}

pub fn discuss(id: &str, note_context: &str, prompt: &[String]) -> Result<()> {
    let config = Config::load()?;
    let user_prompt = if prompt.is_empty() {
        "Continue the discussion from this exact note.".to_string()
    } else {
        prompt.join(" ")
    };
    let skill_bodies = skills::installed_agent_skill_bodies()?;
    let prepared_prompt = build_discuss_prompt(&skill_bodies, id, note_context, &user_prompt);

    run_codex(&config, &prepared_prompt)
}

fn run_codex(config: &Config, prepared_prompt: &str) -> Result<()> {
    eprintln!("agent running codex");

    match config.agent.output {
        AgentOutputMode::Full => run_full(prepared_prompt),
        AgentOutputMode::Format => run_format(prepared_prompt),
        AgentOutputMode::Hidden => run_hidden(prepared_prompt),
    }
}

fn run_full(prompt: &str) -> Result<()> {
    let workspace = nt_home()?;
    let status = ProcessCommand::new("codex")
        .arg("exec")
        .arg(prompt)
        .current_dir(workspace)
        .status()?;

    if !status.success() {
        return Err(NtError::Message(format!("codex exec failed: {status}")));
    }

    Ok(())
}

fn run_format(prompt: &str) -> Result<()> {
    let output = codex_output(prompt)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        if !stderr.trim().is_empty() {
            eprint!("{stderr}");
        }
        return Err(NtError::Message(format!(
            "codex exec failed: {}",
            output.status
        )));
    }

    let formatted = codex_answer(&stdout);
    if !formatted.is_empty() {
        println!("{formatted}");
    }

    Ok(())
}

fn run_hidden(prompt: &str) -> Result<()> {
    let output = codex_output(prompt)?;

    if !output.status.success() {
        return Err(NtError::Message(format!(
            "codex exec failed: {}",
            output.status
        )));
    }

    eprintln!("agent done");
    Ok(())
}

fn codex_output(prompt: &str) -> Result<std::process::Output> {
    let workspace = nt_home()?;
    Ok(ProcessCommand::new("codex")
        .arg("exec")
        .arg(prompt)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .output()?)
}

fn build_agent_prompt(skills: &[(String, String)], user_prompt: &str) -> String {
    let mut prompt = String::from(
        "Use the nt skills below to answer the user request. Run visible nt commands when needed.\n\n",
    );

    push_skills(&mut prompt, skills);

    prompt.push_str("## user request\n\n");
    prompt.push_str(user_prompt);
    prompt
}

fn build_discuss_prompt(
    skills: &[(String, String)],
    id: &str,
    note_context: &str,
    user_prompt: &str,
) -> String {
    let mut prompt = String::from(
        "Use the nt skills below to discuss the exact note context shown here. Do not retrieve additional notes automatically.\n\n",
    );

    push_skills(&mut prompt, skills);

    prompt.push_str("## note context\n\n");
    prompt.push_str("The following context is the visible output of `nt show ");
    prompt.push_str(id);
    prompt.push_str("`.\n\n```text\n");
    prompt.push_str(note_context.trim_end());
    prompt.push_str("\n```\n\n## user request\n\n");
    prompt.push_str(user_prompt);
    prompt
}

fn push_skills(prompt: &mut String, skills: &[(String, String)]) {
    for (name, body) in skills {
        prompt.push_str("## ");
        prompt.push_str(name);
        prompt.push_str("\n\n");
        prompt.push_str(body.trim());
        prompt.push_str("\n\n");
    }
}

pub fn codex_answer(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let Some(start) = lines.iter().rposition(|line| line.trim() == "codex") else {
        return output.trim().to_string();
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == "tokens used")
        .map(|index| start + 1 + index)
        .unwrap_or(lines.len());

    lines[start + 1..end].join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{build_agent_prompt, build_discuss_prompt, codex_answer};

    #[test]
    fn extracts_codex_answer_from_verbose_output() {
        let output = "\
OpenAI Codex v0.133.0
--------
user
hello
codex
answer line

```python
print(4)
```
tokens used
8,680";

        assert_eq!(
            codex_answer(output),
            "answer line\n\n```python\nprint(4)\n```"
        );
    }

    #[test]
    fn agent_prompt_includes_visible_skills_and_request() {
        let prompt = build_agent_prompt(
            &[(
                "nt-recall".to_string(),
                "# nt-recall\n\nUse nt show.".to_string(),
            )],
            "what did I decide?",
        );

        assert!(prompt.contains("Run visible nt commands when needed"));
        assert!(prompt.contains("## nt-recall"));
        assert!(prompt.contains("Use nt show."));
        assert!(prompt.contains("## user request\n\nwhat did I decide?"));
    }

    #[test]
    fn discuss_prompt_uses_exact_note_context_without_auto_retrieval() {
        let note_context = "\
NT20260607T100000  Exact note
path notes/NT20260607T100000.md
created 2026-06-07T10:00:00Z
updated 2026-06-07T10:00:00Z
kind note
status -
tags -
collections -
links -
sources -

# Exact note

Only this note.";
        let prompt = build_discuss_prompt(
            &[("nt-note".to_string(), "# nt-note".to_string())],
            "NT20260607T100000",
            note_context,
            "what should I do next?",
        );

        assert!(prompt.contains("Do not retrieve additional notes automatically"));
        assert!(prompt.contains("`nt show NT20260607T100000`"));
        assert!(prompt.contains("NT20260607T100000  Exact note"));
        assert!(prompt.contains("# Exact note"));
        assert!(prompt.contains("## user request\n\nwhat should I do next?"));
    }
}
