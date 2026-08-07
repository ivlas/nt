use std::io::{self, IsTerminal, Read, Write};
use std::process::Command;

use crate::error::{NtError, Result};
use crate::fs::nt_home;

pub fn read_body(arguments: &[String], seed: Option<&str>) -> Result<String> {
    let stdin_is_terminal = io::stdin().is_terminal();
    if !arguments.is_empty() {
        if !stdin_is_terminal {
            let mut stdin = String::new();
            io::stdin().read_to_string(&mut stdin)?;
            if !stdin.is_empty() {
                return Err(NtError::ConflictingBodyInput);
            }
        }
        return Ok(arguments.join(" "));
    }

    if !stdin_is_terminal {
        let mut body = String::new();
        io::stdin().read_to_string(&mut body)?;
        if body.is_empty() {
            return Err(NtError::EmptyBody);
        }
        return Ok(body);
    }

    read_editor(seed)
}

fn read_editor(seed: Option<&str>) -> Result<String> {
    let visual = std::env::var("VISUAL").ok();
    let editor = std::env::var("EDITOR").ok();
    let editor = parse_editor(visual.as_deref(), editor.as_deref())?;

    let directory = nt_home()?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    if let Some(seed) = seed {
        file.write_all(seed.as_bytes())?;
        file.flush()?;
    }
    let status = Command::new(editor.program)
        .args(editor.arguments)
        .arg(file.path())
        .status()
        .map_err(|_| NtError::EditorFailed)?;
    if !status.success() {
        return Err(NtError::EditorFailed);
    }

    let body = std::fs::read_to_string(file.path())?;
    if body.is_empty() {
        return Err(NtError::EmptyBody);
    }
    Ok(body)
}

#[derive(Debug, Eq, PartialEq)]
struct EditorCommand {
    program: String,
    arguments: Vec<String>,
}

fn parse_editor(visual: Option<&str>, editor: Option<&str>) -> Result<EditorCommand> {
    let value = visual
        .filter(|value| !value.trim().is_empty())
        .or_else(|| editor.filter(|value| !value.trim().is_empty()))
        .ok_or(NtError::EditorNotSet)?;
    let mut arguments = shlex::split(value).ok_or(NtError::InvalidEditor)?;
    if arguments.first().is_none_or(String::is_empty) {
        return Err(NtError::InvalidEditor);
    }
    let program = arguments.remove(0);
    Ok(EditorCommand { program, arguments })
}

#[cfg(test)]
mod tests {
    use super::{EditorCommand, parse_editor};
    use crate::error::NtError;

    #[test]
    fn visual_precedes_editor_and_values_are_parsed_as_argv() {
        assert_eq!(
            parse_editor(Some("code --wait"), Some("vim")).unwrap(),
            EditorCommand {
                program: "code".to_string(),
                arguments: vec!["--wait".to_string()],
            }
        );
        assert_eq!(
            parse_editor(
                None,
                Some("'/Applications/Visual Studio Code.app/code' --wait")
            )
            .unwrap(),
            EditorCommand {
                program: "/Applications/Visual Studio Code.app/code".to_string(),
                arguments: vec!["--wait".to_string()],
            }
        );
        assert_eq!(
            parse_editor(Some(" "), Some("vim -f")).unwrap(),
            EditorCommand {
                program: "vim".to_string(),
                arguments: vec!["-f".to_string()],
            }
        );
        assert_eq!(
            parse_editor(None, Some("code --wait ';' touch /tmp/not-created")).unwrap(),
            EditorCommand {
                program: "code".to_string(),
                arguments: vec![
                    "--wait".to_string(),
                    ";".to_string(),
                    "touch".to_string(),
                    "/tmp/not-created".to_string(),
                ],
            }
        );
    }

    #[test]
    fn empty_and_malformed_editor_commands_are_rejected() {
        assert!(matches!(
            parse_editor(Some(" "), None),
            Err(NtError::EditorNotSet)
        ));
        assert!(matches!(
            parse_editor(None, Some("'unterminated")),
            Err(NtError::InvalidEditor)
        ));
        assert!(matches!(
            parse_editor(None, Some("''")),
            Err(NtError::InvalidEditor)
        ));
    }
}
