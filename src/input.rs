use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

use crate::error::{NtError, Result};

pub struct Input<'a> {
    stdin: &'a mut dyn Read,
    stdin_is_terminal: bool,
    editor: &'a mut dyn FnMut(Option<String>) -> Result<String>,
}

impl<'a> Input<'a> {
    pub fn new(
        stdin: &'a mut dyn Read,
        stdin_is_terminal: bool,
        editor: &'a mut dyn FnMut(Option<String>) -> Result<String>,
    ) -> Self {
        Self {
            stdin,
            stdin_is_terminal,
            editor,
        }
    }

    pub fn read_body(&mut self, arguments: &[String], seed: Option<&str>) -> Result<String> {
        if !arguments.is_empty() {
            if !self.stdin_is_terminal {
                let mut stdin = String::new();
                self.stdin.read_to_string(&mut stdin)?;
                if !stdin.is_empty() {
                    return Err(NtError::ConflictingBodyInput);
                }
            }
            return Ok(arguments.join(" "));
        }

        if !self.stdin_is_terminal {
            let mut body = String::new();
            self.stdin.read_to_string(&mut body)?;
            if body.is_empty() {
                return Err(NtError::EmptyBody);
            }
            return Ok(body);
        }

        (self.editor)(seed.map(str::to_string))
    }
}

pub fn read_editor(
    seed: Option<&str>,
    directory: &Path,
    visual: Option<&str>,
    editor: Option<&str>,
) -> Result<String> {
    let editor = parse_editor(visual, editor)?;
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
    use std::io::{self, Cursor, Read};

    use super::{EditorCommand, Input, parse_editor};
    use crate::error::NtError;

    #[test]
    fn body_input_uses_supplied_reader_and_editor() {
        let mut stdin = Cursor::new("# From stdin");
        let mut editor = |_| panic!("editor should not run");
        let mut input = Input::new(&mut stdin, false, &mut editor);
        assert_eq!(input.read_body(&[], None).unwrap(), "# From stdin");

        let mut stdin = Cursor::new(Vec::new());
        let mut editor = |seed: Option<String>| Ok(format!("{} edited", seed.unwrap()));
        let mut input = Input::new(&mut stdin, true, &mut editor);
        assert_eq!(
            input.read_body(&[], Some("# Original")).unwrap(),
            "# Original edited"
        );
    }

    #[test]
    fn body_input_propagates_reader_failures() {
        let mut stdin = FailingReader;
        let mut editor = |_| panic!("editor should not run");
        let mut input = Input::new(&mut stdin, false, &mut editor);
        assert!(matches!(input.read_body(&[], None), Err(NtError::Io(_))));
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("input failed"))
        }
    }

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
