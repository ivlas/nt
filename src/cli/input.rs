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
    let mut file = tempfile::NamedTempFile::new_in(directory)
        .map_err(|error| NtError::path_io("create editor temporary file in", directory, error))?;
    let file_path = file.path().to_path_buf();
    if let Some(seed) = seed {
        file.write_all(seed.as_bytes())
            .map_err(|error| NtError::path_io("write editor temporary file", &file_path, error))?;
        file.flush()
            .map_err(|error| NtError::path_io("flush editor temporary file", &file_path, error))?;
    }
    let status = Command::new(editor.program)
        .args(editor.arguments)
        .arg(file.path())
        .status()
        .map_err(NtError::EditorLaunch)?;
    if !status.success() {
        return Err(NtError::EditorExit(status));
    }

    let body = std::fs::read_to_string(&file_path)
        .map_err(|error| NtError::path_io("read editor temporary file", &file_path, error))?;
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
    use std::error::Error;
    use std::io::{self, Cursor, Read};

    use super::{EditorCommand, Input, parse_editor, read_editor};
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

    #[cfg(unix)]
    #[test]
    fn editor_returns_successfully_modified_content() {
        let directory = tempfile::tempdir().unwrap();
        let body = read_editor(
            Some("# Original"),
            directory.path(),
            None,
            Some(r##"sh -c 'printf "# Edited\nBody\n" > "$1"' sh"##),
        )
        .unwrap();

        assert_eq!(body, "# Edited\nBody\n");
    }

    #[cfg(unix)]
    #[test]
    fn editor_rejects_a_successfully_emptied_file() {
        let directory = tempfile::tempdir().unwrap();
        let error = read_editor(
            Some("# Original"),
            directory.path(),
            None,
            Some(r#"sh -c ': > "$1"' sh"#),
        )
        .unwrap_err();

        assert!(matches!(error, NtError::EmptyBody));
    }

    #[test]
    fn editor_launch_failures_preserve_the_io_error() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-editor");
        let error = read_editor(
            None,
            directory.path(),
            None,
            Some(missing.to_str().unwrap()),
        )
        .unwrap_err();

        assert!(
            matches!(&error, NtError::EditorLaunch(source) if source.kind() == io::ErrorKind::NotFound)
        );
        assert!(error.source().is_some());
        assert!(error.to_string().starts_with("failed to launch editor:"));
    }

    #[cfg(unix)]
    #[test]
    fn unsuccessful_editor_status_is_reported_separately() {
        let directory = tempfile::tempdir().unwrap();
        let error = read_editor(None, directory.path(), None, Some("false")).unwrap_err();

        assert!(matches!(error, NtError::EditorExit(status) if !status.success()));
    }

    #[test]
    fn editor_temp_file_creation_failures_include_the_directory() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing");

        let error = read_editor(None, &missing, None, Some("unused")).unwrap_err();
        assert!(matches!(
            error,
            NtError::PathIo { path, source, .. }
                if path == missing && source.kind() == io::ErrorKind::NotFound
        ));
    }

    #[cfg(unix)]
    #[test]
    fn editor_file_read_failures_include_the_temporary_path() {
        let directory = tempfile::tempdir().unwrap();
        let removed = read_editor(
            None,
            directory.path(),
            None,
            Some("sh -c 'rm -- \"$1\"' sh"),
        );
        assert!(matches!(
            removed,
            Err(NtError::PathIo { path, source, .. })
                if path.parent() == Some(directory.path())
                    && source.kind() == io::ErrorKind::NotFound
        ));

        let invalid_utf8 = read_editor(
            None,
            directory.path(),
            None,
            Some(r#"sh -c 'printf "\377" > "$1"' sh"#),
        );
        assert!(matches!(
            invalid_utf8,
            Err(NtError::PathIo { path, source, .. })
                if path.parent() == Some(directory.path())
                    && source.kind() == io::ErrorKind::InvalidData
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_editor_is_a_launch_failure() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let editor = directory.path().join("editor");
        std::fs::write(&editor, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = std::fs::metadata(&editor).unwrap().permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(&editor, permissions).unwrap();

        assert!(matches!(
            read_editor(
                None,
                directory.path(),
                None,
                Some(editor.to_str().unwrap())
            ),
            Err(NtError::EditorLaunch(source)) if source.kind() == io::ErrorKind::PermissionDenied
        ));
    }
}
