use std::process::{Command as ProcessCommand, Stdio};

use crate::config::{AgentOutputMode, Config};
use crate::error::{NtError, Result};
use crate::skills;

pub fn run(prompt: &[String]) -> Result<()> {
    let config = Config::load()?;
    if config.agent.backend != "codex" {
        return Err(NtError::Message(format!(
            "unsupported agent backend: {}",
            config.agent.backend
        )));
    }

    let user_prompt = prompt.join(" ");
    let skill_bodies = skills::installed_agent_skill_bodies()?;
    let prepared_prompt = build_prompt(&skill_bodies, &user_prompt);

    eprintln!("agent running codex");

    match config.agent.output {
        AgentOutputMode::Full => run_full(&prepared_prompt),
        AgentOutputMode::Format => run_format(&prepared_prompt),
        AgentOutputMode::Hidden => run_hidden(&prepared_prompt),
    }
}

fn run_full(prompt: &str) -> Result<()> {
    let status = ProcessCommand::new("codex")
        .arg("exec")
        .arg(prompt)
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
    Ok(ProcessCommand::new("codex")
        .arg("exec")
        .arg(prompt)
        .stdin(Stdio::null())
        .output()?)
}

fn build_prompt(skills: &[(String, String)], user_prompt: &str) -> String {
    let mut prompt = String::from(
        "Use the nt skills below to answer the user request. Run visible nt commands when needed.\n\n",
    );

    for (name, body) in skills {
        prompt.push_str("## ");
        prompt.push_str(name);
        prompt.push_str("\n\n");
        prompt.push_str(body.trim());
        prompt.push_str("\n\n");
    }

    prompt.push_str("## user request\n\n");
    prompt.push_str(user_prompt);
    prompt
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
    use super::codex_answer;

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
}
